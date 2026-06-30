from __future__ import annotations

import bpy


class OPENASSET_PT_workspace(bpy.types.Panel):
    bl_label = "OpenAsset Depot"
    bl_idname = "OPENASSET_PT_workspace"
    bl_space_type = "VIEW_3D"
    bl_region_type = "UI"
    bl_category = "OpenAsset"

    def draw(self, context):
        layout = self.layout
        state = context.window_manager.openasset_depot

        status = layout.box()
        if state.busy:
            status.label(text=state.operation_label, icon="FILE_REFRESH")
            if hasattr(status, "progress"):
                status.progress(
                    factor=state.progress / 100.0,
                    type="BAR",
                    text=f"{int(state.progress)}% · {state.message}",
                )
            else:
                status.label(text=f"{int(state.progress)}% · {state.message}")
        else:
            icon = "ERROR" if state.status_kind == "error" else "CHECKMARK" if state.status_kind == "success" else "INFO"
            for index, line in enumerate(_wrapped_status(state.message)):
                status.label(text=line, icon=icon if index == 0 else "NONE")
            if state.needs_reauth:
                status.operator("openasset.open_desktop", text="Open Desktop App to Sign In", icon="URL")
        row = status.row(align=True)
        row.enabled = not state.busy
        row.operator("openasset.refresh", text="Refresh", icon="FILE_REFRESH")
        row.operator("openasset.checkout", text="Check Out", icon="LOCKED")

        actions = layout.column(align=True)
        actions.enabled = not state.busy
        actions.operator("openasset.add", text="Add Current Scene", icon="ADD")
        actions.operator("openasset.sync", text="Sync Latest", icon="IMPORT")
        actions.operator("openasset.validate", text="Validate Scene", icon="CHECKMARK")
        actions.prop(state, "submit_description", text="")
        actions.operator("openasset.submit", text="Submit Changes", icon="EXPORT")
        actions.separator()
        actions.operator("openasset.revert", text="Revert Checkout", icon="LOOP_BACK")


CLASSES = (OPENASSET_PT_workspace,)


def _wrapped_status(message: str, width: int = 42) -> list[str]:
    words = message.split()
    if not words:
        return ["Ready"]
    lines = []
    current = words[0]
    for word in words[1:]:
        candidate = f"{current} {word}"
        if len(candidate) <= width:
            current = candidate
        else:
            lines.append(current)
            current = word
    lines.append(current)
    return lines[:4]
