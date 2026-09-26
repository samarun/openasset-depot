"""Maya smoke test. Run with `mayapy scripts/host-smoke/maya_smoke.py`.

`mayapy` gives a real `maya.cmds` and a real `maya.utils` scheduler without
opening the GUI, which is what makes this worth running: the panel's worker
callbacks go through `maya.utils.executeDeferred`, and that only exists here.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(ROOT / "plugins" / "maya"))

import maya.standalone  # noqa: E402

maya.standalone.initialize(name="python")

import maya.cmds as cmds  # noqa: E402
import maya.utils  # noqa: E402
from harness import FakeBridge, passed, require, require_canonical_labels, wait_until  # noqa: E402

from openasset_depot_maya import runtime, ui  # noqa: E402

HOST = "maya"


def main() -> None:
    require_canonical_labels(ui.ACTIONS.values(), host=HOST)

    scene = Path(cmds.internalVar(userTmpDir=True)) / "oad-smoke.ma"
    cmds.file(rename=str(scene))
    cmds.file(save=True, type="mayaAscii", force=True)
    require(runtime.scene_path() == str(scene), "Maya did not report the saved scene path")

    # The panel has to exist before `run_operation`, which reports progress
    # through it. Building it headless is also the check that no widget in the
    # panel needs a GUI to construct.
    ui.show()
    require(not ui.is_busy(), "the panel started out busy")

    bridge = FakeBridge(local_state="modified")
    results: list[str] = []
    runtime.run_operation(
        lambda: bridge.submit("Maya smoke", timeout_seconds=1800),
        success=lambda result: results.append(str(len(result["revisions"]))) or "Submitted 1 file(s)",
        label="Submitting changes",
    )
    # `executeDeferred` in standalone mode runs on the next idle event, which a
    # blocking script never reaches unless it drains the queue itself.
    wait_until(lambda: bool(results), pump=maya.utils.processIdleEvents)
    require(not ui.is_busy(), "the panel stayed busy after the worker finished")
    require(bridge.called("submit"), "the submit never reached the bridge")

    ui.uninstall()
    runtime.shutdown()
    passed(HOST)


try:
    main()
finally:
    maya.standalone.uninitialize()
