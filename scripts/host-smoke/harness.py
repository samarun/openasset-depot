"""Host-agnostic assertions shared by every in-host smoke test.

A host integration can break in two ways that unit tests outside the host will
never catch: the panel fails to load because the host ships a different Python
or Qt than we assumed, and the worker thread never hands its result back because
the host's main-thread scheduler behaves differently. Both only reproduce inside
the real application, which is why these checks run under `mayapy`, `hython`,
`nuke -t`, and `blender --background` rather than under `python`.

Each host script drives its own panel and calls the functions here, so adding a
host means writing the twenty lines that are genuinely host-specific instead of
reinventing a fake bridge and a set of expectations.
"""

from __future__ import annotations

import sys
import time
from pathlib import Path
from typing import Any, Callable, Iterable

ROOT = Path(__file__).resolve().parents[2]
if str(ROOT / "plugins" / "common") not in sys.path:
    sys.path.insert(0, str(ROOT / "plugins" / "common"))

from openasset_depot_bridge import words  # noqa: E402


class FakeStatus:
    """One file's state, shaped exactly like `BridgeClient.status` returns."""

    def __init__(
        self,
        path: str = "Scenes/Smoke.scene",
        local_state: str = "untracked",
        pending_action: str | None = None,
        needs_sync: bool = False,
        lock_state: str | None = None,
        lock_reason: str | None = None,
    ) -> None:
        self.path = path
        self.local_path = Path(path)
        self.local_state = local_state
        self.pending_action = pending_action
        self.local_revision = None
        self.remote_revision = None
        self.remote_deleted = False
        self.needs_sync = needs_sync
        self.lock_state = lock_state
        self.lock_reason = lock_reason


class FakeBridge:
    """Stands in for the `oad` CLI so a smoke test needs no server.

    The point of an in-host test is the host, not the network. A real server
    round trip is covered by the API integration suite, and putting one here
    would mean a licensed-host runner also needed Postgres.
    """

    def __init__(self, **status_kwargs: Any) -> None:
        self._status_kwargs = status_kwargs
        self.calls: list[tuple[str, tuple[Any, ...]]] = []

    def _record(self, name: str, *args: Any) -> None:
        self.calls.append((name, args))

    def status(self, paths: Iterable[str]) -> list[FakeStatus]:
        self._record("status", tuple(paths))
        return [FakeStatus(path=path, **self._status_kwargs) for path in paths]

    def pending(self) -> dict:
        self._record("pending")
        return {"files": [{"path": "Scenes/Smoke.scene", "action": "add"}]}

    def add(self, path: str) -> dict:
        self._record("add", path)
        return {"path": path}

    def checkout(self, path: str, reason: str | None = None) -> dict:
        self._record("checkout", path, reason)
        return {"path": path}

    def revert(self, path: str) -> dict:
        self._record("revert", path)
        return {"path": path}

    def sync(self, timeout_seconds: int | None = None) -> dict:
        self._record("sync", timeout_seconds)
        return {"synced_count": 3}

    def validate(self, paths: Iterable[str], adapter: str) -> dict:
        self._record("validate", tuple(paths), adapter)
        return {"adapter": {"errors": [], "warnings": []}, "core": {"errors": [], "warnings": []}}

    def submit(self, description: str, timeout_seconds: int | None = None) -> dict:
        self._record("submit", description, timeout_seconds)
        return {"revisions": [{"path": "Scenes/Smoke.scene"}]}

    def shelve(self, timeout_seconds: int | None = None) -> dict:
        self._record("shelve", timeout_seconds)
        return {"files": [{"path": "Scenes/Smoke.scene"}]}

    def unshelve(self, timeout_seconds: int | None = None) -> dict:
        self._record("unshelve", timeout_seconds)
        return {"restored_count": 1}

    def upload_preview(self, path: str, image: str) -> dict:
        self._record("upload_preview", path, image)
        return {"path": path}

    def called(self, name: str) -> bool:
        return any(call == name for call, _ in self.calls)


class SmokeFailure(AssertionError):
    """Raised so the driver can tell a failed assertion from a crashed host."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SmokeFailure(message)


def wait_until(predicate: Callable[[], bool], *, timeout: float = 10.0, pump=None) -> None:
    """Waits for a worker result, pumping the host's callback queue if needed.

    Hosts that deliver callbacks on their own idle loop will never run them
    while a script blocks the main thread, so `pump` lets the caller drain them
    manually. That distinction is the single most common cause of a panel that
    works interactively and hangs in batch.
    """

    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if pump is not None:
            pump()
        if predicate():
            return
        time.sleep(0.02)
    raise SmokeFailure("the host never delivered the worker result")


def canonical_labels() -> set[str]:
    """Every string a panel is allowed to show as a button or field label."""

    return (
        set(words.ACTIONS.values())
        | set(words.STATUSES.values())
        | set(words.FIELDS.values())
        | set(words.PROGRESS.values())
        | {words.PRODUCT_NAME, words.SHORT_NAME}
    )


def require_canonical_labels(labels: Iterable[str], *, host: str) -> None:
    """Fails when a panel shows a label that is not from the word list.

    A host qualifier is allowed — Houdini's "Check Out Selected HDA" — as long
    as it starts with a canonical verb, which is the rule `words.qualified`
    enforces and this mirrors.
    """

    allowed = canonical_labels()
    verbs = tuple(words.ACTIONS.values())
    for label in labels:
        if label in allowed or label.startswith(verbs):
            continue
        raise SmokeFailure(f"{host} shows non-canonical label {label!r}")


def passed(host: str) -> None:
    # The driver greps for this, so an exit code of 0 from a host that quietly
    # skipped the script cannot pass for a successful run.
    print(f"OAD_HOST_SMOKE_PASS {host}")
