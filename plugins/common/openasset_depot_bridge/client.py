"""Dependency-free bridge from DCC Python runtimes to the Rust ``oad`` CLI."""

from __future__ import annotations

import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable, Dict, Iterable, List, Mapping, Optional, Sequence


PROTOCOL_VERSION = 2
DEFAULT_TIMEOUT_SECONDS = 120
LONG_OPERATION_TIMEOUT_SECONDS = 30 * 60
DEFAULT_MAX_OUTPUT_BYTES = 4 * 1024 * 1024


class BridgeError(RuntimeError):
    """An OpenAsset operation failed and is safe to show to an artist."""


class BridgeProtocolError(BridgeError):
    """The CLI response did not match the integration protocol."""


@dataclass(frozen=True)
class OperationProgress:
    operation: str
    phase: str
    message: str
    completed: int
    total: int
    # Per-file transfer detail. These stay None when the CLI does not know a
    # value yet, so panels can distinguish "zero" from "not reported".
    path: Optional[str] = None
    files_completed: Optional[int] = None
    files_total: Optional[int] = None
    bytes_completed: Optional[int] = None
    bytes_total: Optional[int] = None

    @classmethod
    def from_payload(cls, payload: Mapping[str, Any]) -> "OperationProgress":
        def optional_int(key: str) -> Optional[int]:
            value = payload.get(key)
            return None if value is None else int(value)

        path = payload.get("path")
        return cls(
            operation=str(payload.get("operation", "operation")),
            phase=str(payload.get("phase", "working")),
            message=str(payload.get("message", "Working")),
            completed=int(payload.get("completed", 0)),
            total=max(1, int(payload.get("total", 100))),
            path=None if path is None else str(path),
            files_completed=optional_int("files_completed"),
            files_total=optional_int("files_total"),
            bytes_completed=optional_int("bytes_completed"),
            bytes_total=optional_int("bytes_total"),
        )


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
        progress_callback: Optional[Callable[[OperationProgress], None]] = None,
    ) -> None:
        self.workspace = find_workspace(workspace)
        self.cli_path = _resolve_cli(cli_path)
        self.server_url = server_url
        self.timeout_seconds = max(1, int(timeout_seconds))
        self.max_output_bytes = max(1024, int(max_output_bytes))
        self.environment = dict(environment or {})
        self.progress_callback = progress_callback

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

    def shelve(self, timeout_seconds: Optional[int] = None) -> Dict[str, Any]:
        return self._invoke("shelve", timeout_seconds=timeout_seconds)

    def unshelve(self, timeout_seconds: Optional[int] = None) -> Dict[str, Any]:
        return self._invoke("unshelve", timeout_seconds=timeout_seconds)

    def upload_preview(
        self,
        path: os.PathLike[str] | str,
        image: os.PathLike[str] | str,
    ) -> Dict[str, Any]:
        return self._invoke(
            "preview",
            str(Path(path)),
            "--image",
            str(Path(image)),
        )

    def upload_review_proxy(
        self,
        path: os.PathLike[str] | str,
        media: os.PathLike[str] | str,
        frame_rate: Optional[str] = None,
        start_frame: Optional[int] = None,
    ) -> Dict[str, Any]:
        args = [
            "review-proxy",
            str(Path(path)),
            "--media",
            str(Path(media)),
        ]
        if frame_rate:
            args.extend(["--frame-rate", frame_rate])
            if start_frame is not None:
                args.extend(["--start-frame", str(start_frame)])
        return self._invoke(
            *args,
            timeout_seconds=LONG_OPERATION_TIMEOUT_SECONDS,
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
        command.extend(["integration", "--protocol-version", "2", *arguments])
        environment = os.environ.copy()
        environment.update(self.environment)
        effective_timeout = timeout_seconds or self.timeout_seconds
        if arguments and arguments[0] in {"sync", "submit", "shelve", "unshelve"}:
            effective_timeout = max(effective_timeout, LONG_OPERATION_TIMEOUT_SECONDS)
        envelope, stderr, return_code = _run_protocol_v2(
            command,
            cwd=self.workspace,
            environment=environment,
            timeout_seconds=effective_timeout,
            max_output_bytes=self.max_output_bytes,
            progress_callback=self.progress_callback,
        )
        if return_code != 0 or not envelope.get("ok"):
            message = envelope.get("error") or stderr.strip() or "OpenAsset operation failed."
            raise BridgeError(_friendly_error(str(message)))
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


def _friendly_error(message: str) -> str:
    lowered = message.lower()
    if "expiredsignature" in lowered or "signature has expired" in lowered:
        return (
            "Your OpenAsset session expired. Open the OpenAsset Depot desktop app, "
            "sign out, and sign in again; then retry this action."
        )
    if "401 unauthorized" in lowered or "authenticationerror" in lowered:
        return (
            "OpenAsset needs you to sign in again. Open the OpenAsset Depot desktop app, "
            "sign in, and retry this action."
        )
    if "failed to connect" in lowered or "connection refused" in lowered:
        return "OpenAsset cannot reach the server. Check your connection and try again."
    return message


def _resolve_cli(cli_path: Optional[os.PathLike[str] | str]) -> Path:
    configured = cli_path or os.environ.get("OAD_CLI")
    if configured:
        resolved = Path(configured).expanduser().resolve()
        if resolved.is_file():
            return resolved
        raise BridgeError(f"OpenAsset CLI was not found at {resolved}.")
    for candidate in _desktop_cli_candidates():
        if candidate.is_file():
            return candidate.resolve()
    discovered = shutil.which("oad")
    if discovered:
        return Path(discovered).resolve()
    raise BridgeError("OpenAsset CLI is not installed or available on PATH.")


def _desktop_cli_candidates() -> List[Path]:
    home = Path.home()
    if sys.platform == "darwin":
        return [
            Path("/Applications/OpenAsset Depot.app/Contents/MacOS/oad"),
            home / "Applications" / "OpenAsset Depot.app" / "Contents" / "MacOS" / "oad",
        ]
    if sys.platform == "win32":
        roots = [
            os.environ.get("LOCALAPPDATA"),
            os.environ.get("ProgramFiles"),
        ]
        return [Path(root) / "OpenAsset Depot" / "oad.exe" for root in roots if root]
    return [
        home / ".local" / "lib" / "openasset-depot" / "oad",
        Path("/opt/openasset-depot/oad"),
    ]


def _run_protocol_v2(
    command: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    timeout_seconds: int,
    max_output_bytes: int,
    progress_callback: Optional[Callable[[OperationProgress], None]],
) -> tuple[Dict[str, Any], str, int]:
    with tempfile.TemporaryFile() as stderr_file:
        process = subprocess.Popen(
            list(command),
            cwd=str(cwd),
            env=dict(environment),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=stderr_file,
            shell=False,
        )
        if process.stdout is None:
            process.kill()
            raise BridgeProtocolError("OpenAsset CLI stdout could not be captured.")

        lines: queue.Queue[Optional[bytes]] = queue.Queue()

        def read_lines() -> None:
            try:
                for line in iter(process.stdout.readline, b""):
                    lines.put(line)
            finally:
                lines.put(None)

        reader = threading.Thread(target=read_lines, name="openasset-progress", daemon=True)
        reader.start()
        deadline = time.monotonic() + timeout_seconds
        retained_bytes = 0
        result: Optional[Dict[str, Any]] = None
        try:
            stream_open = True
            while stream_open:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise BridgeError(f"OpenAsset operation timed out after {timeout_seconds} seconds.")
                try:
                    line = lines.get(timeout=min(0.1, remaining))
                except queue.Empty:
                    if process.poll() is not None and not reader.is_alive():
                        break
                    continue
                if line is None:
                    stream_open = False
                    continue
                retained_bytes += len(line)
                if retained_bytes > max_output_bytes:
                    raise BridgeProtocolError(
                        f"OpenAsset CLI stdout exceeded the {max_output_bytes}-byte safety limit."
                    )
                payload = _parse_protocol_line(line)
                message_type = payload.get("type")
                if message_type == "progress":
                    if progress_callback:
                        try:
                            progress_callback(OperationProgress.from_payload(payload))
                        except Exception:
                            # UI reporting is optional and must never interrupt a file operation.
                            pass
                elif message_type == "result":
                    result = payload
                else:
                    raise BridgeProtocolError("OpenAsset CLI returned an unsupported protocol message.")

            remaining = max(0.01, deadline - time.monotonic())
            return_code = process.wait(timeout=remaining)
        except (subprocess.TimeoutExpired, BridgeError) as error:
            process.kill()
            process.wait()
            if isinstance(error, subprocess.TimeoutExpired):
                raise BridgeError(f"OpenAsset operation timed out after {timeout_seconds} seconds.") from error
            raise
        finally:
            process.stdout.close()
            reader.join(timeout=1)

        stderr = _read_bounded(stderr_file, max_output_bytes, "stderr")
        if result is None:
            detail = stderr.strip() or "OpenAsset CLI returned no result."
            raise BridgeProtocolError(detail)
        return result, stderr, return_code


def _read_bounded(file_object: Any, limit: int, label: str) -> str:
    file_object.seek(0)
    payload = file_object.read(limit + 1)
    if len(payload) > limit:
        raise BridgeProtocolError(f"OpenAsset CLI {label} exceeded the {limit}-byte safety limit.")
    return payload.decode("utf-8", errors="replace")


def _parse_protocol_line(line: bytes) -> Dict[str, Any]:
    try:
        payload = json.loads(line.decode("utf-8"))
    except json.JSONDecodeError as error:
        raise BridgeProtocolError("OpenAsset CLI did not return valid JSON.") from error
    if not isinstance(payload, dict) or payload.get("protocol_version") != PROTOCOL_VERSION:
        raise BridgeProtocolError("OpenAsset CLI protocol version is not supported.")
    return payload


def _optional_string(value: Any) -> Optional[str]:
    return None if value is None else str(value)


def _optional_int(value: Any) -> Optional[int]:
    return None if value is None else int(value)
