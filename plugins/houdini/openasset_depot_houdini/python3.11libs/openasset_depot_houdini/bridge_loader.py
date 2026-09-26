from __future__ import annotations

import sys
from pathlib import Path


def _add_common_to_path() -> None:
    plugins_root = next(
        (parent for parent in Path(__file__).resolve().parents if (parent / "common").is_dir()),
        None,
    )
    if plugins_root and str(plugins_root / "common") not in sys.path:
        sys.path.insert(0, str(plugins_root / "common"))


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


def load_theme():
    try:
        from openasset_depot_bridge import theme
    except ImportError:
        _add_common_to_path()
        from openasset_depot_bridge import theme
    return theme
