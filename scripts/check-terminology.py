#!/usr/bin/env python3
"""Fails when a host integration reintroduces retired wording.

Nine plugin panels independently drifted apart on the words for the same
operation. `plugins/common/openasset_depot_bridge/words.py` fixed that, and this
check keeps it fixed: a panel that hard-codes "Revert Checkout" again fails CI
with the replacement term named.

Run it directly to see every violation at once:

    ./scripts/check-terminology.py
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "plugins" / "common"))

from openasset_depot_bridge import words  # noqa: E402

# Surfaces an artist reads. Server code and migrations are excluded because their
# strings are protocol values, not user-facing copy.
SEARCH_ROOTS = (
    "plugins",
    "desktop/src",
    "docs",
    "README.md",
)

SEARCH_SUFFIXES = {
    ".py",
    ".js",
    ".jsx",
    ".ts",
    ".tsx",
    ".cs",
    ".cpp",
    ".h",
    ".html",
    ".css",
    ".md",
    ".xml",
}

# Paths that legitimately contain retired wording.
EXCLUDED_PARTS = {
    "node_modules",
    "dist",
    "__pycache__",
    ".git",
    "Binaries",
    "Intermediate",
}

EXCLUDED_FILES = {
    # Defines the retired terms, so it must name them.
    "plugins/common/openasset_depot_bridge/words.py",
    # Explains in a comment why "Revert Intent" beat "Revert Checkout".
    "plugins/common/node/words.js",
    # Asserts on them.
    "plugins/common/tests/test_words.py",
    # This checker.
    "scripts/check-terminology.py",
    # Explains the migration to readers, including the old words.
    "docs/glossary.md",
}

# Unreal's plug-in implements Unreal's own source-control interface, whose
# operation names ("Check In", "Mark for Add") are fixed by the engine. Renaming
# them would break the integration, so the engine's vocabulary wins there and the
# glossary documents the mapping.
UNREAL_PREFIX = "plugins/unreal/"


def relevant_files() -> list[Path]:
    files: list[Path] = []
    for root in SEARCH_ROOTS:
        target = REPO_ROOT / root
        if target.is_file():
            files.append(target)
            continue
        for path in target.rglob("*"):
            if not path.is_file() or path.suffix not in SEARCH_SUFFIXES:
                continue
            if EXCLUDED_PARTS.intersection(path.parts):
                continue
            files.append(path)
    return files


def main() -> int:
    violations: list[str] = []
    for path in sorted(relevant_files()):
        relative = path.relative_to(REPO_ROOT).as_posix()
        if relative in EXCLUDED_FILES or relative.startswith(UNREAL_PREFIX):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for line_number, line in enumerate(text.splitlines(), start=1):
            for retired, replacement in words.RETIRED_TERMS.items():
                # Case-sensitive: these are labels, and a case-insensitive rule
                # would also flag sentences that merely mention the operation.
                if re.search(re.escape(retired), line):
                    violations.append(
                        f"{relative}:{line_number}: {retired!r} is retired; "
                        f"use {replacement!r} from openasset_depot_bridge.words"
                    )

    if violations:
        print("Retired terminology found:\n", file=sys.stderr)
        for violation in violations:
            print(f"  {violation}", file=sys.stderr)
        print(
            f"\n{len(violations)} violation(s). See docs/glossary.md for the "
            "canonical vocabulary.",
            file=sys.stderr,
        )
        return 1

    print(f"Terminology check passed across {len(relevant_files())} files.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
