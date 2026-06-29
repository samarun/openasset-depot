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
        status.label(text=state.message, icon="LOCKED" if "Checked Out" in state.message else "INFO")
        row = status.row(align=True)
        row.enabled = not state.busy
        row.operator("openasset.refresh", text="Refresh", icon="FILE_REFRESH")
        row.operator("openasset.checkout", text="Check Out", icon="LOCKED")

        actions = layout.column(align=True)
        actions.enabled = not state.busy
        actions.operator("openasset.sync", text="Sync Latest", icon="IMPORT")
        actions.operator("openasset.validate", text="Validate Scene", icon="CHECKMARK")
        actions.prop(state, "submit_description", text="")
        actions.operator("openasset.submit", text="Submit Changes", icon="EXPORT")
        actions.separator()
        actions.operator("openasset.revert", text="Revert Checkout", icon="LOOP_BACK")


CLASSES = (OPENASSET_PT_workspace,)
