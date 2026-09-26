"""Blender smoke test.

Run with `blender --background --factory-startup --python
scripts/host-smoke/blender_smoke.py`.

Blender is the one host in the matrix that needs no licence, so this leg runs on
a GitHub-hosted runner on every push and is the early warning for changes that
break the shared bridge for all four Python hosts.

The detailed add-on behaviour is already covered by `scripts/test-blender-addon.py`;
this delegates to it rather than restating it, then adds the checks the whole
matrix shares.
"""

from __future__ import annotations

import runpy
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(ROOT / "plugins" / "blender"))

from harness import passed, require, require_canonical_labels  # noqa: E402

HOST = "blender"


def main() -> None:
    runpy.run_path(str(ROOT / "scripts" / "test-blender-addon.py"), run_name="__main__")

    from openasset_depot_addon import operators

    require_canonical_labels(operators.ACTIONS.values(), host=HOST)
    labels = [
        operator.bl_label
        for operator in operators.CLASSES
        # The desktop-launcher operator is a recovery affordance, not one of the
        # depot operations, so it has no entry in the word list.
        if operator.__name__ != "OPENASSET_OT_open_desktop"
    ]
    require_canonical_labels(labels, host=HOST)
    require(len(labels) >= 7, "the add-on stopped registering its depot operators")

    passed(HOST)


main()
