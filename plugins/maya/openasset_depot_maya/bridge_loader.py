from __future__ import annotations

import sys
from pathlib import Path


def load_bridge():
    try:
        from openasset_depot_bridge import BridgeClient, BridgeError, TaskRunner
    except ImportError:
        common = Path(__file__).resolve().parents[2] / "common"
        if common.is_dir() and str(common) not in sys.path:
            sys.path.insert(0, str(common))
        from openasset_depot_bridge import BridgeClient, BridgeError, TaskRunner
    return BridgeClient, BridgeError, TaskRunner
