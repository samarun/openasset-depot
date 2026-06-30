from __future__ import annotations

from typing import Any, Callable

import maya.cmds as cmds
import maya.utils

from .bridge_loader import load_bridge


BridgeClient, BridgeError, TaskRunner = load_bridge()
_runner = None


def scene_path() -> str:
    path = cmds.file(query=True, sceneName=True)
    if not path:
        raise BridgeError("Save the Maya scene inside an OpenAsset workspace first.")
    return path


def client() -> Any:
    cli_path = cmds.optionVar(query="openassetDepotCli") if cmds.optionVar(exists="openassetDepotCli") else ""
    server_url = (
        cmds.optionVar(query="openassetDepotServer")
        if cmds.optionVar(exists="openassetDepotServer")
        else ""
    )
    return BridgeClient(
        scene_path(),
        cli_path=cli_path or None,
        server_url=server_url or None,
        progress_callback=_progress_from_worker,
    )


def _progress_from_worker(progress: Any) -> None:
    from . import ui

    maya.utils.executeDeferred(lambda progress=progress: ui.set_progress(progress))


def runner():
    global _runner
    if _runner is None:
        _runner = TaskRunner("openasset-maya")
    return _runner


def run_operation(operation: Callable[[], Any], success: Callable[[Any], str], label: str) -> None:
    from . import ui

    if ui.is_busy():
        raise BridgeError("Another OpenAsset operation is already running.")
    ui.set_busy(True)
    ui.set_progress_value(2, label)

    def completed(result: Any) -> None:
        ui.set_busy(False)
        ui.set_progress_value(100, success(result))

    def failed(error: Exception) -> None:
        ui.set_busy(False)
        ui.set_progress_value(0, "")
        ui.set_status(str(error), error=True)

    runner().submit(
        operation,
        schedule=maya.utils.executeDeferred,
        on_success=completed,
        on_error=failed,
    )


def shutdown() -> None:
    global _runner
    if _runner is not None:
        _runner.shutdown()
        _runner = None
