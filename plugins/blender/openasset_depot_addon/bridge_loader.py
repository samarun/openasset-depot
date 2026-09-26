from __future__ import annotations

import sys
from pathlib import Path


def _add_common_to_path() -> None:
    candidates = [
        Path(__file__).resolve().parent / "vendor",
        Path(__file__).resolve().parents[2] / "common",
    ]
    common = next((candidate for candidate in candidates if candidate.is_dir()), None)
    if common and str(common) not in sys.path:
        sys.path.insert(0, str(common))


def load_bridge():
    try:
        from openasset_depot_bridge import BridgeClient, BridgeError, TaskRunner
    except ImportError:
        _add_common_to_path()
        from openasset_depot_bridge import BridgeClient, BridgeError, TaskRunner
    return BridgeClient, BridgeError, TaskRunner


def load_words():
    try:
        from openasset_depot_bridge import words
    except ImportError:
        _add_common_to_path()
        from openasset_depot_bridge import words
    return words
