#!/usr/bin/env python3
"""Register the extracted OpenAsset Depot package with one Houdini install."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--packages-dir",
        required=True,
        type=Path,
        help="Houdini packages directory, for example ~/Library/Preferences/houdini/20.5/packages",
    )
    parser.add_argument(
        "--plugin-root",
        type=Path,
        default=Path(__file__).resolve().parent,
        help="Permanent extracted plug-in directory",
    )
    args = parser.parse_args()
    root = args.plugin_root.expanduser().resolve()
    if not (root / "python_panels" / "openasset_depot.pypanel").is_file():
        raise SystemExit(f"OpenAsset Depot Houdini files were not found at {root}")

    packages_dir = args.packages_dir.expanduser().resolve()
    packages_dir.mkdir(parents=True, exist_ok=True)
    target = packages_dir / "openasset_depot.json"
    temporary = target.with_suffix(".json.tmp")
    payload = {
        "enable": True,
        "env": [{"OPENASSET_DEPOT_HOUDINI_ROOT": str(root)}],
        "path": "$OPENASSET_DEPOT_HOUDINI_ROOT",
    }
    temporary.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    temporary.replace(target)
    print(f"Registered OpenAsset Depot for Houdini in {target}")


if __name__ == "__main__":
    main()
