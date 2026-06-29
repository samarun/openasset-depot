from __future__ import annotations

import nuke

from . import runtime
from .bridge_loader import load_bridge


_, BridgeError, _ = load_bridge()
_busy = False


def refresh() -> None:
    _with_script(
        lambda bridge, path: bridge.status([path]),
        lambda values: _status_message(values[0]),
    )


def refresh_if_panel_open() -> None:
    from .panel import active_panel

    if active_panel() is not None:
        refresh()


def checkout() -> None:
    _with_script(
        lambda bridge, path: bridge.checkout(path, "Editing in Nuke"),
        lambda result: f"Checked out {result['path']}",
    )


def sync() -> None:
    _with_script(
        lambda bridge, _path: bridge.sync(timeout_seconds=1800),
        lambda result: f"Synced {result.get('synced_count', 0)} file(s)",
    )


def validate() -> None:
    _with_script(
        lambda bridge, path: bridge.validate([path], "Nuke"),
        _validation_message,
    )


def submit(description: str = "") -> None:
    description = description.strip()
    if not description:
        description = nuke.getInput("Submit description", "Nuke script update") or ""
    if not description.strip():
        _set_status("Enter a submit description.")
        return
    _with_script(
        lambda bridge, _path: bridge.submit(description.strip(), timeout_seconds=1800),
        lambda result: f"Submitted {len(result.get('revisions', []))} file(s)",
    )


def revert() -> None:
    if not nuke.ask("Remove this script from the pending changelist and release its lock?"):
        return
    _with_script(
        lambda bridge, path: bridge.revert(path),
        lambda result: f"Reverted checkout for {result['path']}",
    )


def _with_script(operation, success) -> None:
    try:
        path = runtime.script_path()
        bridge = runtime.client()
        _run(lambda: operation(bridge, path), success)
    except BridgeError as error:
        _set_status(str(error))


def _run(operation, success) -> None:
    global _busy
    if _busy:
        _set_status("Another OpenAsset operation is already running.")
        return
    _busy = True
    _set_panel_busy(True)
    _set_status("Working...")

    def completed(result) -> None:
        global _busy
        _busy = False
        _set_panel_busy(False)
        _set_status(success(result))

    def failed(error) -> None:
        global _busy
        _busy = False
        _set_panel_busy(False)
        _set_status(str(error))

    runtime.run_operation(operation, on_success=completed, on_error=failed)


def _set_status(message: str) -> None:
    from .panel import active_panel

    panel = active_panel()
    if panel is not None:
        panel.set_status(message)
    else:
        nuke.tprint(f"OpenAsset Depot: {message}")


def _set_panel_busy(busy: bool) -> None:
    from .panel import active_panel

    panel = active_panel()
    if panel is not None:
        panel.set_busy(busy)


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
