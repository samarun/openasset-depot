from __future__ import annotations

import maya.cmds as cmds
import maya.mel as mel

from . import runtime
from .bridge_loader import load_bridge, load_theme, load_words


_, BridgeError, _ = load_bridge()
_words = load_words()
_theme = load_theme()
ACTIONS = _words.ACTIONS
WINDOW = "openassetDepotWindow"
MENU = "openassetDepotMenu"
STATUS = "openassetDepotStatus"
DESCRIPTION = "openassetDepotDescription"
ACTION_COLUMN = "openassetDepotActions"
PROGRESS = "openassetDepotProgress"
_script_jobs = []


def show() -> None:
    if cmds.window(WINDOW, exists=True):
        cmds.deleteUI(WINDOW)
    window = cmds.window(WINDOW, title=_words.PRODUCT_NAME, widthHeight=(360, 410), sizeable=True)
    cmds.columnLayout(adjustableColumn=True, rowSpacing=8, columnAttach=("both", 12))
    cmds.text(label="CURRENT SCENE", align="left", font="smallBoldLabelFont")
    cmds.text(STATUS, label="Save the scene to begin", align="left", wordWrap=True, height=42)
    cmds.progressBar(PROGRESS, maxValue=100, progress=0, height=8, visible=False)
    cmds.separator(style="in")
    cmds.columnLayout(ACTION_COLUMN, adjustableColumn=True, rowSpacing=6)
    cmds.button(label=ACTIONS["refresh"], command=lambda *_: refresh())
    cmds.button(
        label=_words.qualified("checkout", "Scene"),
        command=lambda *_: checkout(),
        # `cmds.button` always draws its label in the host's light text colour,
        # so the accent has to be the dark one to stay readable.
        backgroundColor=_theme.rgb_floats("primary_deep"),
    )
    cmds.button(label=ACTIONS["sync"], command=lambda *_: sync())
    cmds.button(label=ACTIONS["validate"], command=lambda *_: validate())
    cmds.textFieldGrp(
        DESCRIPTION,
        label=_words.FIELDS["description"],
        text="Maya scene update",
        columnWidth2=(80, 240),
    )
    cmds.button(label=ACTIONS["submit"], command=lambda *_: submit())
    cmds.button(label=ACTIONS["shelve"], command=lambda *_: shelve())
    cmds.button(label=ACTIONS["unshelve"], command=lambda *_: unshelve())
    cmds.separator(style="none", height=4)
    cmds.button(label=ACTIONS["revert"], command=lambda *_: revert())
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
    cmds.menuItem(
        label=_words.qualified("checkout", "Current Scene"),
        parent=menu,
        command=lambda *_: checkout(),
    )
    cmds.menuItem(label=ACTIONS["sync"], parent=menu, command=lambda *_: sync())


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
        _words.progress_for("refresh"),
    )


def checkout() -> None:
    _run(
        lambda bridge, path: bridge.checkout(path, "Editing in Maya"),
        lambda result: f"Checked out {result['path']}",
        _words.progress_for("checkout"),
    )


def sync() -> None:
    _run(
        lambda bridge, _path: bridge.sync(timeout_seconds=1800),
        lambda result: f"Synced {result.get('synced_count', 0)} file(s)",
        _words.progress_for("sync"),
    )


def validate() -> None:
    _run(
        lambda bridge, path: bridge.validate([path], "Maya"),
        _validation_message,
        _words.progress_for("validate"),
    )


def submit() -> None:
    description = cmds.textFieldGrp(DESCRIPTION, query=True, text=True).strip()
    if not description:
        set_status(f"Enter a {_words.FIELDS['description'].lower()} first.", error=True)
        return
    _run(
        lambda bridge, _path: bridge.submit(description, timeout_seconds=1800),
        lambda result: f"Submitted {len(result.get('revisions', []))} file(s)",
        _words.progress_for("submit"),
    )


def shelve() -> None:
    _run(
        lambda bridge, _path: bridge.shelve(timeout_seconds=1800),
        lambda result: f"Shelved {len(result.get('files', []))} file(s)",
        _words.progress_for("shelve"),
    )


def unshelve() -> None:
    _run(
        lambda bridge, _path: bridge.unshelve(timeout_seconds=1800),
        lambda result: f"Restored {result.get('restored_count', 0)} file(s)",
        _words.progress_for("unshelve"),
    )


def revert() -> None:
    if not cmds.confirmDialog(
        title=ACTIONS["revert"],
        message="Remove this scene from the pending changelist and release its lock?",
        button=["Revert", "Cancel"],
        defaultButton="Cancel",
        cancelButton="Cancel",
        dismissString="Cancel",
    ) == "Revert":
        return
    _run(
        lambda bridge, path: bridge.revert(path),
        lambda result: f"Reverted {result['path']}",
        _words.progress_for("revert"),
    )


def settings() -> None:
    current_cli = cmds.optionVar(query="openassetDepotCli") if cmds.optionVar(exists="openassetDepotCli") else ""
    current_server = (
        cmds.optionVar(query="openassetDepotServer")
        if cmds.optionVar(exists="openassetDepotServer")
        else ""
    )
    result = cmds.promptDialog(
        title=f"{_words.PRODUCT_NAME} Settings",
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
        title=f"{_words.PRODUCT_NAME} Settings",
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
        color = _theme.rgb_floats("red") if error else _theme.rgb_floats("muted")
        cmds.text(STATUS, edit=True, label=message, font="boldLabelFont", backgroundColor=color)


def set_busy(busy: bool) -> None:
    if cmds.layout(ACTION_COLUMN, exists=True):
        cmds.columnLayout(ACTION_COLUMN, edit=True, enable=not busy)


def set_progress(progress) -> None:
    set_progress_value(progress.completed, progress.message)


def set_progress_value(value: int, message: str) -> None:
    if cmds.control(PROGRESS, exists=True):
        cmds.progressBar(PROGRESS, edit=True, progress=max(0, min(100, int(value))), visible=bool(message))
    if message:
        set_status(message)


def is_busy() -> bool:
    return bool(cmds.layout(ACTION_COLUMN, exists=True) and not cmds.columnLayout(ACTION_COLUMN, query=True, enable=True))


def _run(operation, success, label) -> None:
    try:
        path = runtime.scene_path()
        bridge = runtime.client()
        runtime.run_operation(lambda: operation(bridge, path), success, label)
    except BridgeError as error:
        set_status(str(error), error=True)


def _install_script_jobs(parent: str) -> None:
    _script_jobs.clear()
    for event in ("SceneOpened", "NewSceneOpened"):
        _script_jobs.append(cmds.scriptJob(event=[event, refresh], parent=parent))


def _status_message(status) -> str:
    label = _words.status_from_bridge(status)
    if status.lock_state == "other" and status.lock_reason:
        return f"{label} — {status.lock_reason}"
    return label


def _validation_message(result) -> str:
    errors = len(result.get("adapter", {}).get("errors", [])) + len(
        result.get("core", {}).get("errors", [])
    )
    warnings = len(result.get("adapter", {}).get("warnings", [])) + len(
        result.get("core", {}).get("warnings", [])
    )
    return f"Validation: {errors} error(s), {warnings} warning(s)"
