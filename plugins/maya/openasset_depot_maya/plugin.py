import maya.api.OpenMaya as om

from . import runtime, ui


def initializePlugin(plugin_object):
    om.MFnPlugin(plugin_object, "OpenAsset Depot Contributors", "0.1.2", "Any")
    ui.install_menu()


def uninitializePlugin(_plugin_object):
    ui.uninstall()
    runtime.shutdown()
