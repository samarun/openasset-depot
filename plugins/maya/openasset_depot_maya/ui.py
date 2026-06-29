from __future__ import annotations

import maya.cmds as cmds
import maya.mel as mel

from . import runtime
from .bridge_loader import load_bridge


_, BridgeError, _ = load_bridge()
WINDOW = "openassetDepotWindow"
MENU = "openassetDepotMenu"
STATUS = "openassetDepotStatus"
DESCRIPTION = "openassetDepotDescription"
ACTION_COLUMN = "openassetDepotActions"
_script_jobs = []


def show() -> None:
    if cmds.window(WINDOW, exists=True):
        cmds.deleteUI(WINDOW)
    window = cmds.window(WINDOW, title="OpenAsset Depot", widthHeight=(360, 410), sizeable=True)
    cmds.columnLayout(adjustableColumn=True, rowSpacing=8, columnAttach=("both", 12))
    cmds.text(label="CURRENT SCENE", align="left", font="smallBoldLabelFont")
    cmds.text(STATUS, label="Save the scene to begin", align="left", wordWrap=True, height=42)
    cmds.separator(style="in")
    cmds.columnLayout(ACTION_COLUMN, adjustableColumn=True, rowSpacing=6)
    cmds.button(label="Refresh Status", command=lambda *_: refresh())
    cmds.button(label="Check Out Scene", command=lambda *_: checkout(), backgroundColor=(0.16, 0.42, 0.38))
    cmds.button(label="Sync Latest", command=lambda *_: sync())
    cmds.button(label="Validate Scene", command=lambda *_: validate())
    cmds.textFieldGrp(
        DESCRIPTION,
        label="Description",
        text="Maya scene update",
        columnWidth2=(80, 240),
    )
    cmds.button(label="Submit Changes", command=lambda *_: submit())
    cmds.separator(style="none", height=4)
    cmds.button(label="Revert Checkout", command=lambda *_: revert())
    cmds.setParent("..")
    cmds.separator(style="in")
    cmds.button(label="Settings", command=lambda *_: settings())
    cmds.showWindow(window)
    _install_script_jobs(window)
    refresh()


def install_menu() -> None:
    main_window = mel.eval("$tmpVar=$gMainWindow")
    if cmds.menu(MENU, exists=True):
        cmds.deleteUI(MENU)
    menu = cmds.menu(MENU, label="OpenAsset", parent=main_window, tearOff=False)
    cmds.menuItem(label="Workspace", parent=menu, command=lambda *_: show())
    cmds.menuItem(divider=True, parent=menu)
    cmds.menuItem(label="Check Out Current Scene", parent=menu, command=lambda *_: checkout())
    cmds.menuItem(label="Sync Latest", parent=menu, command=lambda *_: sync())


def uninstall() -> None:
    for job in list(_script_jobs):
        if cmds.scriptJob(exists=job):
            cmds.scriptJob(kill=job, force=True)
    _script_jobs.clear()
    if cmds.window(WINDOW, exists=True):
        cmds.deleteUI(WINDOW)
    if cmds.menu(MENU, exists=True):
        cmds.deleteUI(MENU)


def refresh() -> None:
    _run(
        lambda bridge, path: bridge.status([path]),
        lambda values: _status_message(values[0]),
    )


def checkout() -> None:
    _run(
        lambda bridge, path: bridge.checkout(path, "Editing in Maya"),
        lambda result: f"Checked out {result['path']}",
    )


def sync() -> None:
    _run(
        lambda bridge, _path: bridge.sync(timeout_seconds=1800),
        lambda result: f"Synced {result.get('synced_count', 0)} file(s)",
    )


def validate() -> None:
    _run(
        lambda bridge, path: bridge.validate([path], "Maya"),
        _validation_message,
    )


def submit() -> None:
    description = cmds.textFieldGrp(DESCRIPTION, query=True, text=True).strip()
    if not description:
        set_status("Enter a submit description.", error=True)
        return
    _run(
        lambda bridge, _path: bridge.submit(description, timeout_seconds=1800),
        lambda result: f"Submitted {len(result.get('revisions', []))} file(s)",
    )


def revert() -> None:
    if not cmds.confirmDialog(
        title="Revert Checkout",
        message="Remove this scene from the pending changelist and release its lock?",
        button=["Revert", "Cancel"],
        defaultButton="Cancel",
        cancelButton="Cancel",
        dismissString="Cancel",
    ) == "Revert":
        return
    _run(
        lambda bridge, path: bridge.revert(path),
        lambda result: f"Reverted checkout for {result['path']}",
    )


def settings() -> None:
    current_cli = cmds.optionVar(query="openassetDepotCli") if cmds.optionVar(exists="openassetDepotCli") else ""
    current_server = (
        cmds.optionVar(query="openassetDepotServer")
        if cmds.optionVar(exists="openassetDepotServer")
        else ""
    )
    result = cmds.promptDialog(
        title="OpenAsset Settings",
        message="oad CLI path (leave blank for PATH):",
        text=current_cli,
        button=["Next", "Cancel"],
        defaultButton="Next",
        cancelButton="Cancel",
        dismissString="Cancel",
    )
    if result != "Next":
        return
    cmds.optionVar(stringValue=("openassetDepotCli", cmds.promptDialog(query=True, text=True).strip()))
    result = cmds.promptDialog(
        title="OpenAsset Settings",
        message="Server URL override (optional):",
        text=current_server,
        button=["Save", "Cancel"],
        defaultButton="Save",
        cancelButton="Cancel",
        dismissString="Cancel",
    )
    if result == "Save":
        cmds.optionVar(
            stringValue=("openassetDepotServer", cmds.promptDialog(query=True, text=True).strip())
        )


def set_status(message: str, error: bool = False) -> None:
    if cmds.control(STATUS, exists=True):
        color = (0.75, 0.24, 0.2) if error else (0.7, 0.7, 0.7)
        cmds.text(STATUS, edit=True, label=message, font="boldLabelFont", backgroundColor=color)


def set_busy(busy: bool) -> None:
    if cmds.layout(ACTION_COLUMN, exists=True):
        cmds.columnLayout(ACTION_COLUMN, edit=True, enable=not busy)


def is_busy() -> bool:
    return bool(cmds.layout(ACTION_COLUMN, exists=True) and not cmds.columnLayout(ACTION_COLUMN, query=True, enable=True))


def _run(operation, success) -> None:
    try:
        path = runtime.scene_path()
        bridge = runtime.client()
        runtime.run_operation(lambda: operation(bridge, path), success)
    except BridgeError as error:
        set_status(str(error), error=True)


def _install_script_jobs(parent: str) -> None:
    _script_jobs.clear()
    for event in ("SceneOpened", "NewSceneOpened"):
        _script_jobs.append(cmds.scriptJob(event=[event, refresh], parent=parent))


def _status_message(status) -> str:
    values = [status.local_state.replace("_", " ").title()]
    if status.needs_sync:
        values.append("Needs Sync")
    if status.lock_state == "mine":
        values.append("Checked Out by Me")
    elif status.lock_state == "other":
        values.append("Checked Out Elsewhere")
    return " | ".join(values)


def _validation_message(result) -> str:
    errors = len(result.get("adapter", {}).get("errors", [])) + len(
        result.get("core", {}).get("errors", [])
    )
    warnings = len(result.get("adapter", {}).get("warnings", [])) + len(
        result.get("core", {}).get("warnings", [])
    )
    return f"Validation: {errors} error(s), {warnings} warning(s)"
