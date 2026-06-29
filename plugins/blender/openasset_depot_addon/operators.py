from __future__ import annotations

import bpy

from . import runtime
from .bridge_loader import load_bridge


_, BridgeError, _ = load_bridge()


class OPENASSET_OT_refresh(bpy.types.Operator):
    bl_idname = "openasset.refresh"
    bl_label = "Refresh OpenAsset Status"
    bl_description = "Refresh checkout, sync, and local change status for this scene"

    def execute(self, _context):
        try:
            path = runtime.scene_path()
            bridge = runtime.client()
            runtime.run_operation(
                lambda: bridge.status([path]),
                success=lambda values: _status_message(values[0]),
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_checkout(bpy.types.Operator):
    bl_idname = "openasset.checkout"
    bl_label = "Check Out Scene"
    bl_description = "Lock the current scene and add it to the active changelist"

    def execute(self, _context):
        try:
            path = runtime.scene_path()
            bridge = runtime.client()
            runtime.run_operation(
                lambda: bridge.checkout(path, "Editing in Blender"),
                success=lambda result: f"Checked out {result['path']}",
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_sync(bpy.types.Operator):
    bl_idname = "openasset.sync"
    bl_label = "Sync Latest"
    bl_description = "Download newer workspace files without overwriting local work"

    def execute(self, _context):
        try:
            bridge = runtime.client()
            runtime.run_operation(
                bridge.sync,
                success=lambda result: f"Synced {result.get('synced_count', 0)} file(s)",
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_validate(bpy.types.Operator):
    bl_idname = "openasset.validate"
    bl_label = "Validate Scene"

    def execute(self, _context):
        try:
            path = runtime.scene_path()
            bridge = runtime.client()
            runtime.run_operation(
                lambda: bridge.validate([path], "Blender"),
                success=_validation_message,
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_submit(bpy.types.Operator):
    bl_idname = "openasset.submit"
    bl_label = "Submit Changes"
    bl_description = "Submit all pending files in this workspace"

    def execute(self, context):
        description = context.window_manager.openasset_depot.submit_description.strip()
        if not description:
            self.report({"ERROR"}, "Enter a submit description.")
            return {"CANCELLED"}
        try:
            bridge = runtime.client()
            runtime.run_operation(
                lambda: bridge.submit(description, timeout_seconds=1800),
                success=lambda result: f"Submitted {len(result.get('revisions', []))} file(s)",
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


class OPENASSET_OT_revert(bpy.types.Operator):
    bl_idname = "openasset.revert"
    bl_label = "Revert Checkout"
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
            )
        except BridgeError as error:
            self.report({"ERROR"}, str(error))
            return {"CANCELLED"}
        return {"FINISHED"}


def _status_message(status) -> str:
    parts = [status.local_state.replace("_", " ").title()]
    if status.needs_sync:
        parts.append("Needs Sync")
    if status.lock_state == "mine":
        parts.append("Checked Out by Me")
    elif status.lock_state == "other":
        parts.append("Checked Out Elsewhere")
    return " | ".join(parts)


def _validation_message(result) -> str:
    errors = len(result.get("adapter", {}).get("errors", [])) + len(
        result.get("core", {}).get("errors", [])
    )
    warnings = len(result.get("adapter", {}).get("warnings", [])) + len(
        result.get("core", {}).get("warnings", [])
    )
    return f"Validation: {errors} error(s), {warnings} warning(s)"


CLASSES = (
    OPENASSET_OT_refresh,
    OPENASSET_OT_checkout,
    OPENASSET_OT_sync,
    OPENASSET_OT_validate,
    OPENASSET_OT_submit,
    OPENASSET_OT_revert,
)
