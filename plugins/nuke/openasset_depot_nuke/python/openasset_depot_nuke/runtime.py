from __future__ import annotations

import os
from pathlib import Path
from typing import Any, Callable

import nuke

from .bridge_loader import load_bridge


BridgeClient, BridgeError, TaskRunner = load_bridge()
_runner = TaskRunner("openasset-nuke")


def script_path() -> str:
    path = nuke.root().name()
    if not path or path == "Root" or not Path(path).is_absolute():
        raise BridgeError("Save the Nuke script inside an OpenAsset workspace first.")
    return path


def client() -> Any:
    return BridgeClient(
        script_path(),
        cli_path=os.environ.get("OAD_CLI") or None,
        server_url=os.environ.get("OAD_SERVER_URL") or None,
        progress_callback=_progress_from_worker,
    )


def _progress_from_worker(progress: Any) -> None:
    from . import commands

    schedule(lambda progress=progress: commands.set_progress(progress))


def schedule(callback: Callable[[], None]) -> None:
    nuke.executeInMainThread(callback)


def run_operation(
    operation: Callable[[], Any],
    *,
    on_success: Callable[[Any], None],
    on_error: Callable[[Exception], None],
) -> None:
    _runner.submit(
        operation,
        schedule=schedule,
        on_success=on_success,
        on_error=on_error,
    )
