#!/usr/bin/env python3
"""Fail when release artifacts do not share one semantic version."""

from __future__ import annotations

import argparse
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def json_version(path: str) -> str:
    return json.loads((ROOT / path).read_text(encoding="utf-8"))["version"]


def toml_version(path: str) -> str:
    return regex_version(path, r'^version\s*=\s*"([^"]+)"')


def regex_version(path: str, pattern: str) -> str:
    text = (ROOT / path).read_text(encoding="utf-8")
    match = re.search(pattern, text, re.MULTILINE)
    if not match:
        raise SystemExit(f"version was not found in {path}")
    return match.group(1)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tag", help="Optional release tag, for example v0.1.0")
    args = parser.parse_args()

    adobe_manifest = ET.parse(
        ROOT / "plugins/adobe/OpenAssetDepotCEP/CSXS/manifest.xml"
    ).getroot()
    resolve_manifest = ET.parse(
        ROOT / "plugins/resolve/OpenAssetDepotResolve/manifest.xml"
    ).getroot()
    blender_tuple = regex_version(
        "plugins/blender/openasset_depot_addon/__init__.py",
        r'"version"\s*:\s*\((\d+\s*,\s*\d+\s*,\s*\d+)\)',
    )
    versions = {
        "server": toml_version("server/api/Cargo.toml"),
        "cli": toml_version("cli/oad/Cargo.toml"),
        "rust_sdk": toml_version("sdk/rust/Cargo.toml"),
        "desktop_rust": toml_version("desktop/src-tauri/Cargo.toml"),
        "desktop_ui": json_version("desktop/package.json"),
        "desktop_bundle": json_version("desktop/src-tauri/tauri.conf.json"),
        "typescript_sdk": json_version("sdk/typescript/package.json"),
        "unity": json_version("plugins/unity/OpenAssetDepotUnity/package.json"),
        "resolve": json_version("plugins/resolve/OpenAssetDepotResolve/package.json"),
        "unreal": regex_version(
            "plugins/unreal/OpenAssetDepotSourceControl/OpenAssetDepotSourceControl.uplugin",
            r'"VersionName"\s*:\s*"([^"]+)"',
        ),
        "blender": ".".join(part.strip() for part in blender_tuple.split(",")),
        "maya": regex_version(
            "plugins/maya/openasset_depot_maya/plugin.py",
            r'MFnPlugin\([^\n]+"([0-9]+\.[0-9]+\.[0-9]+)"',
        ),
        "adobe_bundle": adobe_manifest.attrib["ExtensionBundleVersion"],
        "adobe_extension": adobe_manifest.find(".//Extension").attrib["Version"],
        "resolve_manifest": resolve_manifest.findtext(".//Version"),
        "python_bridge": toml_version("plugins/common/pyproject.toml"),
        "integration_archives": regex_version(
            "scripts/package-integrations.py", r'^VERSION\s*=\s*"([^"]+)"'
        ),
    }
    unique = set(versions.values())
    if len(unique) != 1:
        details = "\n".join(f"  {name}: {version}" for name, version in versions.items())
        raise SystemExit(f"release versions do not match:\n{details}")
    version = unique.pop()
    if args.tag and args.tag != f"v{version}":
        raise SystemExit(f"tag {args.tag!r} does not match v{version}")
    print(f"OpenAsset Depot release version {version} is consistent")


if __name__ == "__main__":
    main()
