from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

import bpy

from . import runtime
from .bridge_loader import load_bridge, load_words


_, BridgeError, _ = load_bridge()
_words = load_words()
ACTIONS = _words.ACTIONS


class OPENASSET_OT_open_desktop(bpy.types.Operator):
    bl_idname = "openasset.open_desktop"
    bl_label = f"Open {_words.PRODUCT_NAME} Desktop"
    bl_description = "Open the desktop app so you can renew the shared plug-in session"

    def execute(self, _context):
        try:
            if sys.platform == "darwin":
                result = subprocess.run(
                    ["open", "-a", "OpenAsset Depot"],
                    check=False,
                    capture_output=True,
                    timeout=10,
                )
                if result.returncode != 0:
                    raise RuntimeError("OpenAsset Depot is not installed in Applications")
            elif sys.platform == "win32":
                local_app_data = os.environ.get("LOCALAPPDATA", "")
                candidate = Path(local_app_data) / "OpenAsset Depot" / "OpenAsset Depot.exe"
                if not candidate.is_file():
                    raise RuntimeError("OpenAsset Depot desktop app is not installed")
                os.startfile(candidate)  # type: ignore[attr-defined]
            else:
                executable = shutil.which("openasset-desktop")
                if not executable:
                    raise RuntimeError("OpenAsset Depot desktop app is not installed")
                subprocess.Popen(
                    [executable],
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    start_new_session=True,
                )
        except Exception as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        self.report({"INFO"}, "Sign in to OpenAsset Depot, then return here and retry.")
        return {"FINISHED"}


class OPENASSET_OT_refresh(bpy.types.Operator):
    bl_idname = "openasset.refresh"
    bl_label = ACTIONS["refresh"]
    bl_description = "Refresh checkout, sync, and local change status for this scene"

    def execute(self, _context):
        try:
            path = runtime.scene_path()
            bridge = runtime.client()
            runtime.run_operation(
                lambda: bridge.status([path]),
                success=lambda values: _status_message(values[0]),
                label=_words.progress_for("refresh"),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_checkout(bpy.types.Operator):
    bl_idname = "openasset.checkout"
    bl_label = _words.qualified("checkout", "Scene")
    bl_description = "Lock the current scene and add it to the active changelist"

    def execute(self, _context):
        try:
            path = runtime.scene_path()
            bridge = runtime.client()
            runtime.run_operation(
                lambda: bridge.checkout(path, "Editing in Blender"),
                success=lambda result: f"Checked out {result['path']}",
                label=_words.progress_for("checkout"),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_add(bpy.types.Operator):
    bl_idname = "openasset.add"
    bl_label = ACTIONS["add"]
    bl_description = "Mark this new Blender scene for its first submission"

    def execute(self, _context):
        try:
            path = runtime.scene_path()
            bridge = runtime.client()
            runtime.run_operation(
                lambda: bridge.add(path),
                success=lambda result: f"Added {result['path']} to pending changes",
                label=_words.progress_for("add"),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_sync(bpy.types.Operator):
    bl_idname = "openasset.sync"
    bl_label = ACTIONS["sync"]
    bl_description = "Download newer workspace files without overwriting local work"

    def execute(self, _context):
        try:
            bridge = runtime.client()
            runtime.run_operation(
                bridge.sync,
                success=lambda result: f"Synced {result.get('synced_count', 0)} file(s)",
                label=_words.progress_for("sync"),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_validate(bpy.types.Operator):
    bl_idname = "openasset.validate"
    bl_label = ACTIONS["validate"]

    def execute(self, _context):
        try:
            path = runtime.scene_path()
            bridge = runtime.client()
            runtime.run_operation(
                lambda: bridge.validate([path], "Blender"),
                success=_validation_message,
                label=_words.progress_for("validate"),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_submit(bpy.types.Operator):
    bl_idname = "openasset.submit"
    bl_label = ACTIONS["submit"]
    bl_description = "Submit all pending files in this workspace"

    def execute(self, context):
        description = context.window_manager.openasset_depot.submit_description.strip()
        if not description:
            self.report({"ERROR"}, _words.MESSAGES["need_description"])
            return {"CANCELLED"}
        try:
            path = runtime.scene_path()
            runtime.save_scene_if_dirty()
            preview_path, preview_warning = runtime.generate_scene_preview()
            review_proxy_path, review_proxy_warning = runtime.generate_scene_review_proxy()
            bridge = runtime.client()

            def submit_scene():
                try:
                    statuses = bridge.status([path])
                    status = statuses[0] if statuses else None
                    if status and status.needs_sync:
                        raise BridgeError(_words.MESSAGES["needs_sync_first"])
                    if status and status.lock_state == "other":
                        raise BridgeError(_words.MESSAGES["locked_by_other"])
                    if status and status.local_state == "untracked":
                        bridge.add(path)
                    elif status and status.local_state == "modified" and not status.pending_action:
                        try:
                            bridge.checkout(path, "Automatic checkout from Blender submit")
                        except BridgeError as error:
                            raise BridgeError(
                                "This scene could not be checked out. Another artist may be using it; "
                                f"refresh the status and try again. Details: {error}"
                            ) from error
                    pending = bridge.pending().get("files", [])
                    if not pending:
                        raise BridgeError(
                            "Nothing changed since the last submitted revision."
                        )
                    result = bridge.submit(description, timeout_seconds=1800)
                    result["preview_uploaded"] = False
                    if preview_warning:
                        result["preview_warning"] = preview_warning
                    result["review_proxy_uploaded"] = False
                    if review_proxy_warning:
                        result["review_proxy_warning"] = review_proxy_warning
                    if preview_path:
                        try:
                            bridge.upload_preview(path, preview_path)
                            result["preview_uploaded"] = True
                        except Exception as error:
                            # The asset revision is already committed at this
                            # point. Preview transport must never make a valid
                            # submission appear to have failed.
                            result["preview_warning"] = str(error)
                    if review_proxy_path:
                        try:
                            bridge.upload_review_proxy(path, review_proxy_path)
                            result["review_proxy_uploaded"] = True
                        except Exception as error:
                            result["review_proxy_warning"] = str(error)
                    return result
                finally:
                    if preview_path:
                        Path(preview_path).unlink(missing_ok=True)
                    if review_proxy_path:
                        Path(review_proxy_path).unlink(missing_ok=True)

            runtime.run_operation(
                submit_scene,
                success=_submit_message,
                label=_words.progress_for("submit"),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_shelve(bpy.types.Operator):
    bl_idname = "openasset.shelve"
    bl_label = ACTIONS["shelve"]
    bl_description = "Park pending work on the server without creating a revision"

    def execute(self, _context):
        try:
            runtime.save_scene_if_dirty()
            bridge = runtime.client()
            runtime.run_operation(
                bridge.shelve,
                success=lambda result: f"Shelved {len(result.get('files', []))} file(s)",
                label=_words.progress_for("shelve"),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_unshelve(bpy.types.Operator):
    bl_idname = "openasset.unshelve"
    bl_label = ACTIONS["unshelve"]
    bl_description = "Restore this workspace's shelved work as pending changes"

    def execute(self, _context):
        try:
            bridge = runtime.client()
            runtime.run_operation(
                bridge.unshelve,
                success=lambda result: f"Restored {result.get('restored_count', 0)} file(s)",
                label=_words.progress_for("unshelve"),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_revert(bpy.types.Operator):
    bl_idname = "openasset.revert"
    bl_label = ACTIONS["revert"]
    bl_description = "Remove pending source-control intent and release the scene lock"

    def invoke(self, context, _event):
        return context.window_manager.invoke_confirm(self, _event)

    def execute(self, _context):
        try:
            path = runtime.scene_path()
            bridge = runtime.client()
            runtime.run_operation(
                lambda: bridge.revert(path),
                success=lambda result: f"Reverted checkout for {result['path']}",
                label=_words.progress_for("revert"),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


def _status_message(status) -> str:
    return _words.status_from_bridge(status)


def _validation_message(result) -> str:
    errors = len(result.get("adapter", {}).get("errors", [])) + len(
        result.get("core", {}).get("errors", [])
    )
    warnings = len(result.get("adapter", {}).get("warnings", [])) + len(
        result.get("core", {}).get("warnings", [])
    )
    return f"Validation: {errors} error(s), {warnings} warning(s)"


def _submit_message(result) -> str:
    message = f"Submitted {len(result.get('revisions', []))} file(s)"
    if result.get("preview_uploaded") and result.get("review_proxy_uploaded"):
        return f"{message} · Preview and interactive review ready"
    if result.get("review_proxy_uploaded"):
        return f"{message} · Interactive review ready"
    if result.get("preview_uploaded"):
        return f"{message} · Preview ready"
    if result.get("preview_warning") or result.get("review_proxy_warning"):
        return f"{message} · Preview unavailable"
    return message


CLASSES = (
    OPENASSET_OT_open_desktop,
    OPENASSET_OT_refresh,
    OPENASSET_OT_checkout,
    OPENASSET_OT_add,
    OPENASSET_OT_sync,
    OPENASSET_OT_validate,
    OPENASSET_OT_submit,
    OPENASSET_OT_shelve,
    OPENASSET_OT_unshelve,
    OPENASSET_OT_revert,
)
