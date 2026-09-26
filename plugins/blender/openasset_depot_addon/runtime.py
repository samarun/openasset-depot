from __future__ import annotations

import platform
import os
import tempfile
from pathlib import Path
from typing import Any, Callable

import bpy

from .bridge_loader import load_bridge


BridgeClient, BridgeError, TaskRunner = load_bridge()
from openasset_depot_bridge import CallbackQueue


_runner = None
_callbacks = CallbackQueue()
_timer_active = False


def task_runner():
    global _runner
    if _runner is None:
        _runner = TaskRunner("openasset-blender")
    return _runner


def scene_path() -> str:
    path = bpy.data.filepath
    if not path:
        raise BridgeError("Save the Blender scene inside an OpenAsset workspace first.")
    return path


def client() -> Any:
    preferences = bpy.context.preferences.addons[__package__].preferences
    cli_path = preferences.cli_path.strip() or bundled_cli_path()
    server_url = preferences.server_url.strip() or None
    return BridgeClient(
        scene_path(),
        cli_path=cli_path,
        server_url=server_url,
        progress_callback=_progress_from_worker,
    )


def _progress_from_worker(progress: Any) -> None:
    schedule(lambda progress=progress: _apply_progress(progress))


def _apply_progress(progress: Any) -> None:
    state = getattr(bpy.context.window_manager, "openasset_depot", None)
    if state is None or not state.busy:
        return
    state.operation_label = _operation_label(getattr(progress, "operation", "operation"))
    state.progress = min(100.0, max(0.0, float(getattr(progress, "completed", 0))))
    state.message = str(getattr(progress, "message", state.operation_label))


def _operation_label(operation: str) -> str:
    return {
        "status": "Refreshing status",
        "checkout": "Checking out scene",
        "add": "Adding scene",
        "sync": "Syncing latest files",
        "validate": "Validating scene",
        "submit": "Submitting changes",
        "preview": "Uploading preview",
        "review_proxy": "Uploading 3D review",
        "revert": "Reverting checkout",
    }.get(operation, "Working")


def bundled_cli_path() -> str | None:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "darwin":
        system = "macos"
    if machine in {"aarch64", "arm64"}:
        machine = "arm64"
    elif machine in {"amd64", "x86_64"}:
        machine = "amd64"
    executable = "oad.exe" if system == "windows" else "oad"
    candidate = Path(__file__).resolve().parent / "bin" / f"{system}-{machine}" / executable
    return str(candidate) if candidate.is_file() else None


def schedule(callback: Callable[[], None]) -> None:
    # Future completion runs on a worker thread. Blender's Python API, including
    # bpy.app.timers.register, may only be called from Blender's main thread.
    _callbacks.schedule(callback)


def _drain_callbacks():
    global _timer_active
    try:
        _callbacks.drain()
    except Exception as error:
        state = getattr(bpy.context.window_manager, "openasset_depot", None)
        if state is not None:
            state.busy = False
            state.status_kind = "error"
            state.needs_reauth = False
            state.message = f"Operation failed: {error}"

    current_runner = _runner
    if not _callbacks.empty or (current_runner is not None and current_runner.pending_count):
        return 0.05

    _timer_active = False
    return None


def _ensure_callback_timer() -> None:
    global _timer_active
    if _timer_active:
        return
    _timer_active = True
    bpy.app.timers.register(_drain_callbacks, first_interval=0.05)


def save_scene_if_dirty() -> None:
    path = scene_path()
    if not bpy.data.is_dirty:
        return
    try:
        result = bpy.ops.wm.save_as_mainfile(filepath=path)
    except Exception as error:
        raise BridgeError(f"Blender could not save the scene: {error}") from error
    if "FINISHED" not in result:
        raise BridgeError("Blender could not save the scene before submitting it.")


def generate_scene_preview() -> tuple[Path | None, str | None]:
    addon = bpy.context.preferences.addons.get(__package__)
    if addon is not None and not addon.preferences.generate_previews:
        return None, None
    scene = bpy.context.scene
    if scene.camera is None:
        return None, "No active camera; submitted without a preview"

    descriptor, raw_path = tempfile.mkstemp(prefix="openasset-preview-", suffix=".png")
    os.close(descriptor)
    preview_path = Path(raw_path)
    preview_path.unlink(missing_ok=True)
    render = scene.render
    image_settings = render.image_settings
    original = {
        "engine": render.engine,
        "filepath": render.filepath,
        "resolution_x": render.resolution_x,
        "resolution_y": render.resolution_y,
        "resolution_percentage": render.resolution_percentage,
        "use_file_extension": render.use_file_extension,
        "file_format": image_settings.file_format,
        "color_mode": image_settings.color_mode,
    }
    try:
        render.engine = (
            "BLENDER_EEVEE_NEXT"
            if (4, 2, 0) <= bpy.app.version < (5, 0, 0)
            else "BLENDER_EEVEE"
        )
        render.filepath = str(preview_path)
        render.resolution_x = 640
        render.resolution_y = 360
        render.resolution_percentage = 100
        render.use_file_extension = False
        image_settings.file_format = "PNG"
        image_settings.color_mode = "RGB"
        result = bpy.ops.render.render(write_still=True)
        if "FINISHED" not in result or not preview_path.is_file():
            preview_path.unlink(missing_ok=True)
            return None, "Preview render did not produce an image"
        return preview_path, None
    except Exception as error:
        preview_path.unlink(missing_ok=True)
        return None, f"Preview unavailable: {error}"
    finally:
        render.engine = original["engine"]
        render.filepath = original["filepath"]
        render.resolution_x = original["resolution_x"]
        render.resolution_y = original["resolution_y"]
        render.resolution_percentage = original["resolution_percentage"]
        render.use_file_extension = original["use_file_extension"]
        image_settings.file_format = original["file_format"]
        image_settings.color_mode = original["color_mode"]


def generate_scene_review_proxy() -> tuple[Path | None, str | None]:
    addon = bpy.context.preferences.addons.get(__package__)
    if addon is not None and not addon.preferences.generate_review_proxy:
        return None, None
    descriptor, raw_path = tempfile.mkstemp(prefix="openasset-review-", suffix=".glb")
    os.close(descriptor)
    proxy_path = Path(raw_path)
    proxy_path.unlink(missing_ok=True)
    try:
        result = bpy.ops.export_scene.gltf(
            filepath=str(proxy_path),
            export_format="GLB",
            export_animations=True,
            export_yup=True,
        )
        if "FINISHED" not in result or not proxy_path.is_file():
            proxy_path.unlink(missing_ok=True)
            return None, "Interactive review export did not produce a GLB"
        return proxy_path, None
    except Exception as error:
        proxy_path.unlink(missing_ok=True)
        return None, f"Interactive review unavailable: {error}"


def run_operation(
    operation: Callable[[], Any],
    *,
    success: Callable[[Any], str],
    label: str,
) -> None:
    state = bpy.context.window_manager.openasset_depot
    if state.busy:
        raise BridgeError("Another OpenAsset operation is already running.")
    state.busy = True
    state.status_kind = "working"
    state.needs_reauth = False
    state.operation_label = label
    state.progress = 2.0
    state.message = f"{label}…"
    _ensure_callback_timer()

    def completed(result: Any) -> None:
        current = bpy.context.window_manager.openasset_depot
        current.busy = False
        current.progress = 100.0
        current.status_kind = "success"
        current.needs_reauth = False
        current.message = success(result)

    def failed(error: Exception) -> None:
        current = bpy.context.window_manager.openasset_depot
        current.busy = False
        current.progress = 0.0
        current.status_kind = "error"
        current.message = str(error)
        current.needs_reauth = "sign in" in current.message.lower()

    try:
        task_runner().submit(operation, schedule=schedule, on_success=completed, on_error=failed)
    except Exception as error:
        state.busy = False
        raise BridgeError(f"Could not start the OpenAsset operation: {error}") from error


def shutdown() -> None:
    global _runner, _timer_active
    if _runner is not None:
        _runner.shutdown()
        _runner = None
    if bpy.app.timers.is_registered(_drain_callbacks):
        bpy.app.timers.unregister(_drain_callbacks)
    _timer_active = False
    _callbacks.clear()
