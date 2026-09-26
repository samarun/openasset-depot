import bpy

from . import operators, panel, runtime
from .bridge_loader import load_words


FIELDS = load_words().FIELDS

bl_info = {
    "name": "OpenAsset Depot",
    "author": "OpenAsset Depot Contributors",
    "version": (0, 1, 2),
    "blender": (3, 6, 0),
    "location": "3D View > Sidebar > OpenAsset",
    "description": "Production source control for Blender scenes and assets",
    "category": "Pipeline",
}


class OpenAssetDepotPreferences(bpy.types.AddonPreferences):
    bl_idname = __package__

    cli_path: bpy.props.StringProperty(
        name="CLI override (optional)",
        description="Leave blank to use the CLI bundled with this add-on",
        subtype="FILE_PATH",
    )
    server_url: bpy.props.StringProperty(
        name="Server override (optional)",
        description="Optional server override; leave blank to use the CLI configuration",
    )
    generate_previews: bpy.props.BoolProperty(
        name="Generate browser previews",
        description="Render a lightweight camera preview and attach it to each submitted revision",
        default=True,
    )
    generate_review_proxy: bpy.props.BoolProperty(
        name="Generate interactive 3D review proxy",
        description="Export an animated GLB after submit for browser orbit, playback, keyframe, and annotation review",
        default=True,
    )

    def draw(self, _context):
        layout = self.layout
        layout.prop(self, "cli_path")
        layout.prop(self, "server_url")
        layout.prop(self, "generate_previews")
        layout.prop(self, "generate_review_proxy")


class OpenAssetDepotState(bpy.types.PropertyGroup):
    message: bpy.props.StringProperty(default="Save the scene to begin")
    submit_description: bpy.props.StringProperty(
        name=FIELDS["description"],
        default="Blender scene update",
    )
    busy: bpy.props.BoolProperty(default=False)
    operation_label: bpy.props.StringProperty(default="Ready")
    progress: bpy.props.FloatProperty(default=0.0, min=0.0, max=100.0)
    status_kind: bpy.props.StringProperty(default="info")
    needs_reauth: bpy.props.BoolProperty(default=False)


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
