import bpy

from . import operators, panel, runtime


bl_info = {
    "name": "OpenAsset Depot",
    "author": "OpenAsset Depot Contributors",
    "version": (0, 1, 0),
    "blender": (3, 6, 0),
    "location": "3D View > Sidebar > OpenAsset",
    "description": "Production source control for Blender scenes and assets",
    "category": "Pipeline",
}


class OpenAssetDepotPreferences(bpy.types.AddonPreferences):
    bl_idname = __package__

    cli_path: bpy.props.StringProperty(
        name="oad CLI",
        description="Optional path to the OpenAsset Depot CLI",
        subtype="FILE_PATH",
    )
    server_url: bpy.props.StringProperty(
        name="Server URL",
        description="Optional server override; leave blank to use the CLI configuration",
    )

    def draw(self, _context):
        layout = self.layout
        layout.prop(self, "cli_path")
        layout.prop(self, "server_url")


class OpenAssetDepotState(bpy.types.PropertyGroup):
    message: bpy.props.StringProperty(default="Save the scene to begin")
    submit_description: bpy.props.StringProperty(
        name="Submit Description",
        default="Blender scene update",
    )
    busy: bpy.props.BoolProperty(default=False)


CLASSES = (
    OpenAssetDepotPreferences,
    OpenAssetDepotState,
    *operators.CLASSES,
    *panel.CLASSES,
)


def register():
    for cls in CLASSES:
        bpy.utils.register_class(cls)
    bpy.types.WindowManager.openasset_depot = bpy.props.PointerProperty(type=OpenAssetDepotState)


def unregister():
    runtime.shutdown()
    del bpy.types.WindowManager.openasset_depot
    for cls in reversed(CLASSES):
        bpy.utils.unregister_class(cls)
