#!/usr/bin/env python3
"""Build self-contained host integration archives and checksums."""

from __future__ import annotations

import hashlib
import platform
import shutil
import tarfile
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PLUGINS = ROOT / "plugins"
OUTPUT = ROOT / "dist" / "integrations"
COMMON_PYTHON = PLUGINS / "common" / "openasset_depot_bridge"
COMMON_NODE = PLUGINS / "common" / "node" / "openasset-cli.js"
COMMON_WORDS = PLUGINS / "common" / "node" / "words.js"
COMMON_PANEL_CSS = PLUGINS / "common" / "visual-kit" / "panel.css"
EXCLUDED_PARTS = {"__pycache__", ".DS_Store"}
VERSION = "0.1.2"


def source_files(root: Path):
    for path in sorted(root.rglob("*")):
        if path.is_file() and not any(part in EXCLUDED_PARTS for part in path.parts):
            yield path


def add_tree(archive: zipfile.ZipFile, source: Path, prefix: str) -> None:
    for path in source_files(source):
        archive.write(path, str(Path(prefix) / path.relative_to(source)))


def blender_platform_tag() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "darwin":
        system = "macos"
    if machine in {"aarch64", "arm64"}:
        machine = "arm64"
    elif machine in {"amd64", "x86_64"}:
        machine = "amd64"
    return f"{system}-{machine}"


def zip_blender() -> list[Path]:
    target = OUTPUT / f"openasset-depot-blender-{VERSION}.zip"
    platform_tag = blender_platform_tag()
    platform_target = OUTPUT / f"openasset-depot-blender-{platform_tag}-{VERSION}.zip"
    source = PLUGINS / "blender" / "openasset_depot_addon"
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        add_tree(archive, source, "openasset_depot_addon")
        add_tree(archive, COMMON_PYTHON, "openasset_depot_addon/vendor/openasset_depot_bridge")
    cli_name = "oad.exe" if platform.system().lower() == "windows" else "oad"
    cli_binary = ROOT / "target" / "release" / cli_name
    if not cli_binary.is_file():
        return [target]
    shutil.copyfile(target, platform_target)
    with zipfile.ZipFile(platform_target, "a", zipfile.ZIP_DEFLATED) as archive:
        archive.write(
            cli_binary,
            f"openasset_depot_addon/bin/{platform_tag}/{cli_name}",
        )
    return [target, platform_target]


def zip_maya() -> Path:
    target = OUTPUT / f"openasset-depot-maya-{VERSION}.zip"
    source = PLUGINS / "maya" / "openasset_depot_maya"
    module_version = ".".join(VERSION.split(".")[:2])
    module = f"""+ OpenAssetDepot {module_version} OpenAssetDepot
PYTHONPATH +:= OpenAssetDepot/scripts
MAYA_PLUG_IN_PATH +:= OpenAssetDepot/plug-ins
"""
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("OpenAssetDepot.mod", module)
        for path in source_files(source):
            relative = path.relative_to(source)
            if relative.parts[0] == "plug-ins" or relative.name == "OpenAssetDepot.mod":
                continue
            if relative.name == "README.md":
                archive.write(path, "OpenAssetDepot/README.md")
            else:
                archive.write(path, str(Path("OpenAssetDepot/scripts/openasset_depot_maya") / relative))
        add_tree(archive, source / "plug-ins", "OpenAssetDepot/plug-ins")
        add_tree(archive, COMMON_PYTHON, "OpenAssetDepot/scripts/openasset_depot_bridge")
    return target


def zip_python_host(name: str, source: Path, python_prefix: str) -> Path:
    target = OUTPUT / f"openasset-depot-{name}-{VERSION}.zip"
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        add_tree(archive, source, source.name)
        add_tree(archive, COMMON_PYTHON, str(Path(source.name) / python_prefix / "openasset_depot_bridge"))
    return target


def zip_source(name: str, source: Path) -> Path:
    target = OUTPUT / f"openasset-depot-{name}-{VERSION}.zip"
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        add_tree(archive, source, source.name)
    return target


def add_shared_web_kit(archive: zipfile.ZipFile, prefix: str) -> None:
    """Embeds the files the HTML panels share instead of duplicating."""

    archive.write(COMMON_NODE, f"{prefix}/lib/openasset-cli.js")
    archive.write(COMMON_WORDS, f"{prefix}/lib/words.js")
    archive.write(COMMON_PANEL_CSS, f"{prefix}/lib/panel.css")


def zip_adobe() -> Path:
    target = OUTPUT / f"openasset-depot-adobe-cep-{VERSION}.zip"
    source = PLUGINS / "adobe" / "OpenAssetDepotCEP"
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        add_tree(archive, source, "com.openassetdepot.creativecloud")
        add_shared_web_kit(archive, "com.openassetdepot.creativecloud")
    return target


def zip_resolve() -> Path:
    target = OUTPUT / f"openasset-depot-resolve-{VERSION}.zip"
    source = PLUGINS / "resolve" / "OpenAssetDepotResolve"
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        add_tree(archive, source, "com.openassetdepot.resolve")
        add_shared_web_kit(archive, "com.openassetdepot.resolve")
    return target


def tar_unity() -> Path:
    target = OUTPUT / f"com.openassetdepot.unity-{VERSION}.tgz"
    source = PLUGINS / "unity" / "OpenAssetDepotUnity"
    with tarfile.open(target, "w:gz", format=tarfile.PAX_FORMAT) as archive:
        for path in source_files(source):
            archive.add(path, str(Path("package") / path.relative_to(source)), recursive=False)
    return target


def checksum_file(paths: list[Path]) -> None:
    lines = []
    for path in sorted(paths):
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        lines.append(f"{digest}  {path.name}")
    (OUTPUT / "SHA256SUMS").write_text("\n".join(lines) + "\n", encoding="ascii")


def main() -> None:
    if OUTPUT.exists():
        shutil.rmtree(OUTPUT)
    OUTPUT.mkdir(parents=True)
    artifacts = [
        *zip_blender(),
        zip_maya(),
        zip_python_host(
            "houdini",
            PLUGINS / "houdini" / "openasset_depot_houdini",
            "python3.11libs",
        ),
        zip_python_host(
            "nuke",
            PLUGINS / "nuke" / "openasset_depot_nuke",
            "python",
        ),
        zip_source("unreal", PLUGINS / "unreal" / "OpenAssetDepotSourceControl"),
        tar_unity(),
        zip_adobe(),
        zip_resolve(),
    ]
    checksum_file(artifacts)
    for artifact in artifacts:
        print(artifact.relative_to(ROOT))


if __name__ == "__main__":
    main()
