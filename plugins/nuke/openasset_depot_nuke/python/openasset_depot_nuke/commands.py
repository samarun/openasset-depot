from __future__ import annotations

import nuke

from . import runtime
from .bridge_loader import load_bridge, load_words


_, BridgeError, _ = load_bridge()
_words = load_words()
_busy = False


def refresh() -> None:
    _with_script(
        lambda bridge, path: bridge.status([path]),
        lambda values: _status_message(values[0]),
        _words.progress_for("refresh"),
    )


def refresh_if_panel_open() -> None:
    from .panel import active_panel

    if active_panel() is not None:
        refresh()


def checkout() -> None:
    _with_script(
        lambda bridge, path: bridge.checkout(path, "Editing in Nuke"),
        lambda result: f"Checked out {result['path']}",
        _words.progress_for("checkout", "script"),
    )


def sync() -> None:
    _with_script(
        lambda bridge, _path: bridge.sync(timeout_seconds=1800),
        lambda result: f"Synced {result.get('synced_count', 0)} file(s)",
        _words.progress_for("sync"),
    )


def validate() -> None:
    _with_script(
        lambda bridge, path: bridge.validate([path], "Nuke"),
        _validation_message,
        _words.progress_for("validate"),
    )


def submit(description: str = "") -> None:
    description = description.strip()
    if not description:
        description = nuke.getInput(_words.FIELDS["description"], "Nuke script update") or ""
    if not description.strip():
        _set_status(_words.MESSAGES["need_description"])
        return
    _with_script(
        lambda bridge, _path: bridge.submit(description.strip(), timeout_seconds=1800),
        lambda result: f"Submitted {len(result.get('revisions', []))} file(s)",
        _words.progress_for("submit"),
    )


def shelve() -> None:
    _with_script(
        lambda bridge, _path: bridge.shelve(timeout_seconds=1800),
        lambda result: f"Shelved {len(result.get('files', []))} file(s)",
        _words.progress_for("shelve"),
    )


def unshelve() -> None:
    _with_script(
        lambda bridge, _path: bridge.unshelve(timeout_seconds=1800),
        lambda result: f"Restored {result.get('restored_count', 0)} file(s)",
        _words.progress_for("unshelve"),
    )


def revert() -> None:
    if not nuke.ask("Remove this script from the pending changelist and release its lock?"):
        return
    _with_script(
        lambda bridge, path: bridge.revert(path),
        lambda result: f"Reverted {result['path']}",
        _words.progress_for("revert"),
    )


def _with_script(operation, success, label) -> None:
    try:
        path = runtime.script_path()
        bridge = runtime.client()
        _run(lambda: operation(bridge, path), success, label)
    except BridgeError as error:
        _set_status(str(error))


def _run(operation, success, label) -> None:
    global _busy
    if _busy:
        _set_status(f"Another {_words.SHORT_NAME} operation is already running.")
        return
    _busy = True
    _set_panel_busy(True)
    _set_progress_value(2)
    _set_status(label)

    def completed(result) -> None:
        global _busy
        _busy = False
        _set_panel_busy(False)
        _set_progress_value(100)
        _set_status(success(result))

    def failed(error) -> None:
        global _busy
        _busy = False
        _set_panel_busy(False)
        _set_progress_value(0)
        _set_status(str(error))

    runtime.run_operation(operation, on_success=completed, on_error=failed)


def set_progress(progress) -> None:
    _set_progress_value(progress.completed)
    _set_status(progress.message)


def _set_progress_value(value: int) -> None:
    from .panel import active_panel

    panel = active_panel()
    if panel is not None:
        panel.set_progress(value)


def _set_status(message: str) -> None:
    from .panel import active_panel

    panel = active_panel()
    if panel is not None:
        panel.set_status(message)
    else:
        nuke.tprint(f"{_words.PRODUCT_NAME}: {message}")


def _set_panel_busy(busy: bool) -> None:
    from .panel import active_panel

    panel = active_panel()
    if panel is not None:
        panel.set_busy(busy)


def _status_message(status) -> str:
    return _words.status_from_bridge(status)


def _validation_message(result) -> str:
    errors = len(result.get("adapter", {}).get("errors", [])) + len(
        result.get("core", {}).get("errors", [])
    )
    warnings = len(result.get("adapter", {}).get("warnings", [])) + len(
        result.get("core", {}).get("warnings", [])
    )
    return f"Validation: {errors} error(s), {warnings} warning(s)"
