#!/usr/bin/env python3
"""Runs a host integration's smoke test inside the real host application.

Most of the hosts we integrate with are commercial software that cannot be
installed on a GitHub-hosted runner: Maya, Houdini, and Nuke all need a licence
server. The result was that the only integration ever exercised in a real host
was whichever one a developer happened to have open, and breakage surfaced as a
bug report from a studio.

This driver makes "run the panel inside the host" a single command, so the same
check runs on a developer's machine and on a studio's self-hosted runner without
either needing to remember the batch-mode incantation for each application.

Exit codes are deliberate:

* 0  - the host ran the smoke test and it passed
* 1  - the host ran the smoke test and it failed, or the host crashed
* 77 - the host is not installed here, so nothing was verified

77 lets a CI matrix leg report "not licensed on this runner" without turning the
build red and without pretending the host passed. See `docs/host-ci.md`.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Sequence

ROOT = Path(__file__).resolve().parent.parent
SMOKE_DIR = ROOT / "scripts" / "host-smoke"
PASS_SENTINEL = "OAD_HOST_SMOKE_PASS"
SKIP_EXIT = 77


@dataclass(frozen=True)
class Host:
    """How to find one application and how to make it run a Python file."""

    name: str
    # Executable names to look for on PATH, in preference order.
    executables: Sequence[str]
    # Extra absolute locations to try, for applications whose installers do not
    # touch PATH. Globs are expanded so a version number does not have to be
    # hard-coded.
    search_globs: Sequence[str]
    # Builds the full argument list, given the resolved executable and script.
    command: Callable[[str, Path], list[str]]
    licence_note: str

    @property
    def script(self) -> Path:
        return SMOKE_DIR / f"{self.name}_smoke.py"

    @property
    def env_override(self) -> str:
        return f"OAD_{self.name.upper()}_EXECUTABLE"


HOSTS: dict[str, Host] = {
    host.name: host
    for host in (
        Host(
            name="blender",
            executables=("blender",),
            search_globs=(
                "/Applications/Blender.app/Contents/MacOS/Blender",
                "/Applications/Blender*.app/Contents/MacOS/Blender",
                "C:/Program Files/Blender Foundation/Blender*/blender.exe",
            ),
            # --factory-startup so a developer's own add-ons and preferences
            # cannot make a broken build look fine, or a fine build look broken.
            command=lambda exe, script: [
                exe,
                "--background",
                "--factory-startup",
                "--python-exit-code",
                "1",
                "--python",
                str(script),
            ],
            licence_note="free; runs on GitHub-hosted runners",
        ),
        Host(
            name="maya",
            executables=("mayapy",),
            search_globs=(
                "/Applications/Autodesk/maya*/Maya.app/Contents/bin/mayapy",
                "/usr/autodesk/maya*/bin/mayapy",
                "C:/Program Files/Autodesk/Maya*/bin/mayapy.exe",
            ),
            command=lambda exe, script: [exe, str(script)],
            licence_note="needs an Autodesk licence reachable from the runner",
        ),
        Host(
            name="houdini",
            executables=("hython",),
            search_globs=(
                "/Applications/Houdini/Houdini*/Frameworks/Houdini.framework/Versions/Current/Resources/bin/hython",
                "/opt/hfs*/bin/hython",
                "C:/Program Files/Side Effects Software/Houdini*/bin/hython.exe",
            ),
            command=lambda exe, script: [exe, str(script)],
            licence_note="needs a SideFX licence; Houdini Apprentice is sufficient",
        ),
        Host(
            name="nuke",
            executables=("nuke", "Nuke"),
            search_globs=(
                "/Applications/Nuke*/Nuke*.app/Contents/MacOS/Nuke*",
                "/usr/local/Nuke*/Nuke*",
                "C:/Program Files/Nuke*/Nuke*.exe",
            ),
            # -t is terminal mode and --nukex/--studio are deliberately not used:
            # the integration must work on a plain Nuke licence.
            command=lambda exe, script: [exe, "-t", str(script)],
            licence_note="needs a Foundry licence; Nuke Non-commercial is sufficient",
        ),
    )
}


def resolve_executable(host: Host) -> str | None:
    override = os.environ.get(host.env_override)
    if override:
        # An explicit override that is wrong is a configuration error, not a
        # missing host, so this is reported rather than silently skipped.
        if not Path(override).is_file():
            raise SystemExit(f"{host.env_override} points at {override}, which is not a file")
        return override
    for name in host.executables:
        found = shutil.which(name)
        if found:
            return found
    for pattern in host.search_globs:
        anchor = Path(pattern)
        if not anchor.is_absolute():
            continue
        matches = sorted(Path(anchor.anchor).glob(str(anchor.relative_to(anchor.anchor))))
        # Newest install wins, which is what a studio upgrading mid-show wants.
        for match in reversed(matches):
            if match.is_file():
                return str(match)
    return None


def run(host: Host) -> int:
    executable = resolve_executable(host)
    if not executable:
        print(f"skip: {host.name} is not installed here ({host.licence_note})")
        print(f"      set {host.env_override} to point at it directly")
        return SKIP_EXIT
    if not host.script.is_file():
        raise SystemExit(f"missing smoke script: {host.script}")

    command = host.command(executable, host.script)
    print(f"running {host.name} smoke test: {' '.join(command)}", flush=True)
    completed = subprocess.run(
        command,
        cwd=ROOT,
        capture_output=True,
        text=True,
        # Hosts that fail to acquire a licence tend to block rather than exit.
        timeout=int(os.environ.get("OAD_HOST_SMOKE_TIMEOUT", "600")),
    )
    sys.stdout.write(completed.stdout)
    sys.stderr.write(completed.stderr)

    if f"{PASS_SENTINEL} {host.name}" not in completed.stdout:
        # Several hosts exit 0 after refusing to run a script, so the exit code
        # alone cannot be trusted to mean the test actually ran.
        print(f"FAIL: {host.name} did not report a passing smoke test")
        return 1
    if completed.returncode != 0:
        print(f"FAIL: {host.name} passed its checks but exited {completed.returncode}")
        return 1
    print(f"ok: {host.name}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--host",
        choices=sorted(HOSTS),
        action="append",
        help="host to test; repeatable. Defaults to every host installed here.",
    )
    parser.add_argument(
        "--require-host",
        action="store_true",
        help="treat a missing host as a failure instead of a skip",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="report which hosts can be found on this machine",
    )
    arguments = parser.parse_args()

    if arguments.list:
        for host in HOSTS.values():
            found = resolve_executable(host)
            print(f"{host.name:9} {found or '-- not installed --'}")
        return 0

    selected = [HOSTS[name] for name in (arguments.host or sorted(HOSTS))]
    worst = 0
    for host in selected:
        code = run(host)
        if code == SKIP_EXIT and arguments.require_host:
            print(f"FAIL: {host.name} was required but is not installed")
            code = 1
        # A real failure always outranks a skip in the summary exit code.
        if code == 1:
            worst = 1
        elif code == SKIP_EXIT and worst == 0:
            worst = SKIP_EXIT
    return worst


if __name__ == "__main__":
    raise SystemExit(main())
