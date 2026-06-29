from __future__ import annotations

from typing import Any, Callable

import bpy

from .bridge_loader import load_bridge


BridgeClient, BridgeError, TaskRunner = load_bridge()
_runner = None


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
    cli_path = preferences.cli_path.strip() or None
    server_url = preferences.server_url.strip() or None
    return BridgeClient(scene_path(), cli_path=cli_path, server_url=server_url)


def schedule(callback: Callable[[], None]) -> None:
    def run_once():
        callback()
        return None

    bpy.app.timers.register(run_once, first_interval=0.0)


def run_operation(
    operation: Callable[[], Any],
    *,
    success: Callable[[Any], str],
) -> None:
    state = bpy.context.window_manager.openasset_depot
    if state.busy:
        raise BridgeError("Another OpenAsset operation is already running.")
    state.busy = True
    state.message = "Working..."

    def completed(result: Any) -> None:
        current = bpy.context.window_manager.openasset_depot
        current.busy = False
        current.message = success(result)

    def failed(error: Exception) -> None:
        current = bpy.context.window_manager.openasset_depot
        current.busy = False
        current.message = str(error)

    task_runner().submit(operation, schedule=schedule, on_success=completed, on_error=failed)


def shutdown() -> None:
    global _runner
    if _runner is not None:
        _runner.shutdown()
        _runner = None
