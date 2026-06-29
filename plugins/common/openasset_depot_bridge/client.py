"""Dependency-free bridge from DCC Python runtimes to the Rust ``oad`` CLI."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Iterable, List, Mapping, Optional, Sequence


PROTOCOL_VERSION = 1
DEFAULT_TIMEOUT_SECONDS = 120
LONG_OPERATION_TIMEOUT_SECONDS = 30 * 60
DEFAULT_MAX_OUTPUT_BYTES = 4 * 1024 * 1024


class BridgeError(RuntimeError):
    """An OpenAsset operation failed and is safe to show to an artist."""


class BridgeProtocolError(BridgeError):
    """The CLI response did not match the integration protocol."""


@dataclass(frozen=True)
class WorkspaceContext:
    workspace_id: str
    depot: str
    stream: str
    root: Path
    active_changelist_id: Optional[str]
    tracked_file_count: int
    pending_file_count: int
    server_url: str
    username: Optional[str]
    authenticated: bool

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "WorkspaceContext":
        return cls(
            workspace_id=str(payload["workspace_id"]),
            depot=str(payload["depot"]),
            stream=str(payload["stream"]),
            root=Path(str(payload["root"])),
            active_changelist_id=_optional_string(payload.get("active_changelist_id")),
            tracked_file_count=int(payload.get("tracked_file_count", 0)),
            pending_file_count=int(payload.get("pending_file_count", 0)),
            server_url=str(payload["server_url"]),
            username=_optional_string(payload.get("username")),
            authenticated=bool(payload.get("authenticated", False)),
        )


@dataclass(frozen=True)
class FileStatus:
    path: str
    local_path: Path
    local_state: str
    pending_action: Optional[str]
    local_revision: Optional[int]
    remote_revision: Optional[int]
    remote_deleted: bool
    needs_sync: bool
    lock_state: Optional[str]
    lock_reason: Optional[str]

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "FileStatus":
        return cls(
            path=str(payload["path"]),
            local_path=Path(str(payload.get("local_path", payload["path"]))),
            local_state=str(payload["local_state"]),
            pending_action=_optional_string(payload.get("pending_action")),
            local_revision=_optional_int(payload.get("local_revision")),
            remote_revision=_optional_int(payload.get("remote_revision")),
            remote_deleted=bool(payload.get("remote_deleted", False)),
            needs_sync=bool(payload.get("needs_sync", False)),
            lock_state=_optional_string(payload.get("lock_state")),
            lock_reason=_optional_string(payload.get("lock_reason")),
        )


class BridgeClient:
    """Runs the stable ``oad integration`` protocol without shell expansion."""

    def __init__(
        self,
        workspace: os.PathLike[str] | str,
        *,
        cli_path: Optional[os.PathLike[str] | str] = None,
        server_url: Optional[str] = None,
        timeout_seconds: int = DEFAULT_TIMEOUT_SECONDS,
        max_output_bytes: int = DEFAULT_MAX_OUTPUT_BYTES,
        environment: Optional[Mapping[str, str]] = None,
    ) -> None:
        self.workspace = find_workspace(workspace)
        self.cli_path = _resolve_cli(cli_path)
        self.server_url = server_url
        self.timeout_seconds = max(1, int(timeout_seconds))
        self.max_output_bytes = max(1024, int(max_output_bytes))
        self.environment = dict(environment or {})

    def context(self) -> WorkspaceContext:
        return WorkspaceContext.from_payload(self._invoke("context"))

    def pending(self) -> Dict[str, Any]:
        return self._invoke("pending")

    def status(self, paths: Iterable[os.PathLike[str] | str]) -> List[FileStatus]:
        normalized = [str(Path(path)) for path in paths]
        if not normalized:
            raise BridgeError("A host integration must request status for one or more files.")
        payload = self._invoke("status", *normalized)
        return [FileStatus.from_payload(item) for item in payload.get("files", [])]

    def checkout(self, path: os.PathLike[str] | str, reason: Optional[str] = None) -> Dict[str, Any]:
        args = ["checkout", str(Path(path))]
        if reason:
            args.extend(["--reason", reason])
        return self._invoke(*args)

    def add(self, path: os.PathLike[str] | str) -> Dict[str, Any]:
        return self._invoke("add", str(Path(path)))

    def delete(self, path: os.PathLike[str] | str, reason: Optional[str] = None) -> Dict[str, Any]:
        args = ["delete", str(Path(path))]
        if reason:
            args.extend(["--reason", reason])
        return self._invoke(*args)

    def lock(self, path: os.PathLike[str] | str, reason: Optional[str] = None) -> Dict[str, Any]:
        args = ["lock", str(Path(path))]
        if reason:
            args.extend(["--reason", reason])
        return self._invoke(*args)

    def unlock(self, path: os.PathLike[str] | str) -> Dict[str, Any]:
        return self._invoke("unlock", str(Path(path)))

    def revert(self, path: os.PathLike[str] | str) -> Dict[str, Any]:
        return self._invoke("revert", str(Path(path)))

    def sync(self, timeout_seconds: Optional[int] = None) -> Dict[str, Any]:
        return self._invoke("sync", timeout_seconds=timeout_seconds)

    def submit(self, description: str, timeout_seconds: Optional[int] = None) -> Dict[str, Any]:
        description = description.strip()
        if not description:
            raise BridgeError("A submit description is required.")
        return self._invoke(
            "submit",
            "--description",
            description,
            timeout_seconds=timeout_seconds,
        )

    def history(self, path: os.PathLike[str] | str) -> Dict[str, Any]:
        return self._invoke("history", str(Path(path)))

    def validate(
        self,
        paths: Sequence[os.PathLike[str] | str],
        adapter: Optional[str] = None,
    ) -> Dict[str, Any]:
        if not paths:
            raise BridgeError("A host integration must validate one or more files.")
        args = ["validate", *(str(Path(path)) for path in paths)]
        if adapter:
            args.extend(["--adapter", adapter])
        return self._invoke(*args)

    def _invoke(
        self,
        *arguments: str,
        timeout_seconds: Optional[int] = None,
    ) -> Dict[str, Any]:
        command = [str(self.cli_path)]
        if self.server_url:
            command.extend(["--server", self.server_url])
        command.extend(["integration", *arguments])
        environment = os.environ.copy()
        environment.update(self.environment)
        effective_timeout = timeout_seconds or self.timeout_seconds
        if arguments and arguments[0] in {"sync", "submit"}:
            effective_timeout = max(effective_timeout, LONG_OPERATION_TIMEOUT_SECONDS)
        stdout, stderr, return_code = _run_bounded(
            command,
            cwd=self.workspace,
            environment=environment,
            timeout_seconds=effective_timeout,
            max_output_bytes=self.max_output_bytes,
        )
        envelope = _parse_envelope(stdout)
        if return_code != 0 or not envelope.get("ok"):
            message = envelope.get("error") or stderr.strip() or "OpenAsset operation failed."
            raise BridgeError(str(message))
        data = envelope.get("data")
        if not isinstance(data, dict):
            raise BridgeProtocolError("OpenAsset CLI returned an invalid data payload.")
        return data


def find_workspace(start: os.PathLike[str] | str) -> Path:
    path = Path(start).expanduser()
    if path.is_file():
        path = path.parent
    path = path.resolve()
    for candidate in (path, *path.parents):
        if (candidate / ".oad" / "workspace.json").is_file():
            return candidate
    raise BridgeError(f"{path} is not inside an OpenAsset Depot workspace.")


def _resolve_cli(cli_path: Optional[os.PathLike[str] | str]) -> Path:
    configured = cli_path or os.environ.get("OAD_CLI")
    if configured:
        resolved = Path(configured).expanduser().resolve()
        if resolved.is_file():
            return resolved
        raise BridgeError(f"OpenAsset CLI was not found at {resolved}.")
    discovered = shutil.which("oad")
    if discovered:
        return Path(discovered).resolve()
    raise BridgeError("OpenAsset CLI is not installed or available on PATH.")


def _run_bounded(
    command: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    timeout_seconds: int,
    max_output_bytes: int,
) -> tuple[str, str, int]:
    with tempfile.TemporaryFile() as stdout_file, tempfile.TemporaryFile() as stderr_file:
        process = subprocess.Popen(
            list(command),
            cwd=str(cwd),
            env=dict(environment),
            stdin=subprocess.DEVNULL,
            stdout=stdout_file,
            stderr=stderr_file,
            shell=False,
        )
        try:
            return_code = process.wait(timeout=timeout_seconds)
        except subprocess.TimeoutExpired as error:
            process.kill()
            process.wait()
            raise BridgeError(f"OpenAsset operation timed out after {timeout_seconds} seconds.") from error
        stdout = _read_bounded(stdout_file, max_output_bytes, "stdout")
        stderr = _read_bounded(stderr_file, max_output_bytes, "stderr")
        return stdout, stderr, return_code


def _read_bounded(file_object: Any, limit: int, label: str) -> str:
    file_object.seek(0)
    payload = file_object.read(limit + 1)
    if len(payload) > limit:
        raise BridgeProtocolError(f"OpenAsset CLI {label} exceeded the {limit}-byte safety limit.")
    return payload.decode("utf-8", errors="replace")


def _parse_envelope(stdout: str) -> Dict[str, Any]:
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as error:
        raise BridgeProtocolError("OpenAsset CLI did not return valid JSON.") from error
    if not isinstance(payload, dict) or payload.get("protocol_version") != PROTOCOL_VERSION:
        raise BridgeProtocolError("OpenAsset CLI protocol version is not supported.")
    return payload


def _optional_string(value: Any) -> Optional[str]:
    return None if value is None else str(value)


def _optional_int(value: Any) -> Optional[int]:
    return None if value is None else int(value)
