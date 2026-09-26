"""Nuke smoke test. Run with `nuke -t scripts/host-smoke/nuke_smoke.py`.

`nuke -t` is terminal mode: real `nuke` module, no GUI. Panels cannot be built
there, so this drives the command layer instead, which is the part that talks to
the bridge and marshals results through `nuke.executeInMainThread`.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(ROOT / "plugins" / "nuke" / "openasset_depot_nuke" / "python"))

import nuke  # noqa: E402
from harness import FakeBridge, passed, require, require_canonical_labels, wait_until  # noqa: E402

from openasset_depot_nuke import commands, runtime  # noqa: E402

HOST = "nuke"


def main() -> None:
    from openasset_depot_nuke import panel

    require_canonical_labels(panel.ACTIONS.values(), host=HOST)

    script = Path("/tmp/oad-smoke.nk")
    nuke.scriptSaveAs(str(script), overwrite=1)
    require(runtime.script_path() == str(script), "Nuke did not report the saved script path")

    bridge = FakeBridge(local_state="modified")
    delivered: list[str] = []
    commands._set_status = delivered.append  # type: ignore[assignment]

    runtime.run_operation(
        lambda: bridge.submit("Nuke smoke", timeout_seconds=1800),
        on_success=lambda result: delivered.append(f"Submitted {len(result['revisions'])} file(s)"),
        on_error=lambda error: delivered.append(f"error: {error}"),
    )
    # In terminal mode `executeInMainThread` runs inline, so no pump is needed;
    # the wait is here to fail loudly rather than hang if that ever changes.
    wait_until(lambda: bool(delivered))
    require(delivered[-1] == "Submitted 1 file(s)", f"unexpected result: {delivered}")
    require(bridge.called("submit"), "the submit never reached the bridge")

    passed(HOST)


main()
