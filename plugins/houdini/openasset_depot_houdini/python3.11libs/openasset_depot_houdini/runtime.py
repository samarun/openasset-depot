from __future__ import annotations

from pathlib import Path
from typing import Any, Callable, Optional

import hdefereval
import hou

from .bridge_loader import load_bridge


BridgeClient, BridgeError, TaskRunner = load_bridge()
_runner = None


def scene_path() -> str:
    path = hou.hipFile.path()
    if not path or Path(path).name.lower().startswith("untitled"):
        raise BridgeError("Save the Houdini scene inside an OpenAsset workspace first.")
    return path


def selected_hda_path() -> str:
    selected = hou.selectedNodes()
    if len(selected) != 1:
        raise BridgeError("Select one node backed by an HDA library.")
    definition = selected[0].type().definition()
    if definition is None or not definition.libraryFilePath():
        raise BridgeError("The selected node is not backed by an HDA library.")
    return definition.libraryFilePath()


def client(path: Optional[str] = None) -> Any:
    cli_path = hou.getenv("OAD_CLI") or None
    server_url = hou.getenv("OAD_SERVER_URL") or None
    return BridgeClient(path or scene_path(), cli_path=cli_path, server_url=server_url)


def runner():
    global _runner
    if _runner is None:
        _runner = TaskRunner("openasset-houdini")
    return _runner


def run_operation(
    operation: Callable[[], Any],
    *,
    on_success: Callable[[Any], None],
    on_error: Callable[[Exception], None],
) -> None:
    runner().submit(
        operation,
        schedule=hdefereval.executeDeferred,
        on_success=on_success,
        on_error=on_error,
    )


def shutdown() -> None:
    global _runner
    if _runner is not None:
        _runner.shutdown()
        _runner = None
