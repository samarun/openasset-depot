"""Houdini smoke test. Run with `hython scripts/host-smoke/houdini_smoke.py`.

`hython` ships the same `hou` module and the same PySide build Houdini itself
uses, so this is where a Qt import that only exists in one Houdini version, or a
stylesheet that Qt refuses to parse, actually shows up.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(
    0,
    str(ROOT / "plugins" / "houdini" / "openasset_depot_houdini" / "python3.11libs"),
)

import hou  # noqa: E402
from harness import FakeBridge, passed, require, require_canonical_labels, wait_until  # noqa: E402

from openasset_depot_houdini import panel, runtime  # noqa: E402

HOST = "houdini"


def main() -> None:
    require_canonical_labels(panel.ACTIONS.values(), host=HOST)

    scene = Path(hou.getenv("TEMP") or "/tmp") / "oad-smoke.hip"
    hou.hipFile.save(str(scene))
    require(runtime.scene_path() == str(scene), "Houdini did not report the saved HIP path")

    # A QApplication is required before any QWidget. Houdini's GUI provides one;
    # hython does not, so the test creates it rather than skipping the widget,
    # because widget construction is most of what can break here.
    from PySide6 import QtWidgets  # type: ignore[import-not-found]

    application = QtWidgets.QApplication.instance() or QtWidgets.QApplication([])

    widget = panel.create_interface()
    require(widget.objectName() == "OadPanel", "the panel lost the object name its stylesheet needs")
    require(bool(widget.styleSheet()), "the shared Qt stylesheet was not applied")
    labels = [button.text() for button in widget.findChildren(QtWidgets.QPushButton)]
    require_canonical_labels(labels, host=HOST)

    bridge = FakeBridge(local_state="modified")
    widget.show()
    widget._run(
        lambda: bridge.submit("Houdini smoke", timeout_seconds=1800),
        lambda result: f"Submitted {len(result['revisions'])} file(s)",
        "Submitting changes",
    )
    wait_until(
        lambda: bridge.called("submit") and not widget._busy,
        pump=application.processEvents,
    )
    require("Submitted 1 file(s)" in widget.status.text(), "the panel never showed the result")

    widget.close()
    passed(HOST)


main()
