"""Guards the shared vocabulary and visual kit against silent drift.

The whole point of `words.py` is that nine hosts agree. These tests fail when the
Node copy diverges, when a status loses its colour, or when the status mapping
stops matching the desktop app's precedence.
"""

from __future__ import annotations

import json
import re
import subprocess
import unittest
from pathlib import Path
from types import SimpleNamespace

from openasset_depot_bridge import theme, words

REPO_ROOT = Path(__file__).resolve().parents[3]
NODE_WORDS = REPO_ROOT / "plugins" / "common" / "node" / "words.js"
PANEL_CSS = REPO_ROOT / "plugins" / "common" / "visual-kit" / "panel.css"
DESKTOP_DOMAIN = REPO_ROOT / "desktop" / "src" / "types" / "domain.ts"
UNITY_WORDS = (
    REPO_ROOT / "plugins" / "unity" / "OpenAssetDepotUnity" / "Editor" / "OpenAssetWords.cs"
)
UNITY_OVERLAY = UNITY_WORDS.with_name("OpenAssetStatusOverlay.cs")
GLOSSARY = REPO_ROOT / "docs" / "glossary.md"


def _node_words() -> dict:
    """Reads the Node word list by asking Node for it, not by parsing it."""

    script = (
        "const w = require(process.argv[1]);"
        "process.stdout.write(JSON.stringify({"
        "PRODUCT_NAME: w.PRODUCT_NAME, SHORT_NAME: w.SHORT_NAME,"
        "ACTIONS: w.ACTIONS, STATUSES: w.STATUSES, PROGRESS: w.PROGRESS,"
        "FIELDS: w.FIELDS, MESSAGES: w.MESSAGES}));"
    )
    result = subprocess.run(
        ["node", "-e", script, str(NODE_WORDS)],
        capture_output=True,
        text=True,
        check=True,
    )
    return json.loads(result.stdout)


class WordListTests(unittest.TestCase):
    def test_python_and_node_word_lists_are_identical(self):
        node = _node_words()
        self.assertEqual(node["PRODUCT_NAME"], words.PRODUCT_NAME)
        self.assertEqual(node["SHORT_NAME"], words.SHORT_NAME)
        self.assertEqual(node["ACTIONS"], dict(words.ACTIONS))
        self.assertEqual(node["STATUSES"], dict(words.STATUSES))
        self.assertEqual(node["PROGRESS"], dict(words.PROGRESS))
        self.assertEqual(node["FIELDS"], dict(words.FIELDS))
        self.assertEqual(node["MESSAGES"], dict(words.MESSAGES))

    def test_the_unity_copy_only_contains_canonical_wording(self):
        # C# cannot import either word list, so the Unity constants are a hand
        # copy. Every value has to appear in the Python module, which is what
        # stops that copy from becoming a third dialect.
        source = UNITY_WORDS.read_text(encoding="utf-8")
        constants = dict(
            re.findall(r'const string (\w+) = "([^"]*)";', source)
        )
        self.assertEqual(constants["ProductName"], words.PRODUCT_NAME)
        self.assertEqual(constants["ShortName"], words.SHORT_NAME)
        canonical = (
            set(words.ACTIONS.values())
            | set(words.STATUSES.values())
            | set(words.FIELDS.values())
            | {words.PRODUCT_NAME, words.SHORT_NAME}
        )
        for name, value in constants.items():
            self.assertIn(value, canonical, f"OpenAssetWords.{name} is not canonical")

    def test_every_action_and_status_is_documented_in_the_glossary(self):
        # A term artists see with no entry explaining it is how the vocabulary
        # drifted in the first place.
        glossary = GLOSSARY.read_text(encoding="utf-8")
        for label in list(words.ACTIONS.values()) + list(words.STATUSES.values()):
            self.assertIn(f"**{label}**", glossary, f"{label} is undocumented")
        for retired in words.RETIRED_TERMS:
            self.assertIn(retired, glossary, f"{retired} is not listed as retired")

    def test_statuses_match_the_desktop_app_exactly(self):
        # The desktop's StatusKind union is what docs/ui.md documents to artists,
        # so a plugin inventing a ninth status would be inventing a status the
        # product does not have.
        source = DESKTOP_DOMAIN.read_text(encoding="utf-8")
        union = re.search(
            r"export type StatusKind =\s*(.*?);", source, re.DOTALL
        )
        self.assertIsNotNone(union, "StatusKind union not found in domain.ts")
        desktop_statuses = set(re.findall(r'"([^"]+)"', union.group(1)))
        self.assertEqual(desktop_statuses, set(words.STATUSES.values()))

    def test_every_status_has_a_colour(self):
        for label in words.STATUSES.values():
            self.assertIn(label, theme.STATUS_COLORS, f"{label} has no colour")
            self.assertTrue(theme.status_color(label).startswith("#"))

    def test_retired_terms_do_not_point_at_themselves(self):
        # A retired term mapped to itself would make the CI check unfixable.
        for retired, replacement in words.RETIRED_TERMS.items():
            self.assertNotEqual(retired, replacement)

    def test_qualified_keeps_the_canonical_verb_in_front(self):
        self.assertEqual(words.qualified("checkout", "Selected HDA"), "Check Out Selected HDA")
        self.assertEqual(words.qualified("checkout", "  "), "Check Out")
        with self.assertRaises(KeyError):
            words.qualified("teleport", "HIP")

    def test_progress_text_names_the_target_when_given_one(self):
        self.assertEqual(words.progress_for("sync"), "Downloading files")
        self.assertEqual(words.progress_for("checkout", "shot.hip"), "Checking out shot.hip")


class StatusPrecedenceTests(unittest.TestCase):
    def test_a_validation_block_outranks_everything(self):
        label = words.status_label(
            blocked=True, lock_state="self", pending_action="edit", needs_sync=True
        )
        self.assertEqual(label, "Blocked")

    def test_another_artists_lock_outranks_the_callers_own_edit(self):
        # The caller can resolve their own pending edit; they cannot resolve
        # somebody else's checkout, so that is the status worth showing.
        label = words.status_label(lock_state="other", pending_action="edit")
        self.assertEqual(label, "In Use")

    def test_pending_actions_map_to_their_own_labels(self):
        self.assertEqual(words.status_label(pending_action="delete"), "Marked for Delete")
        self.assertEqual(words.status_label(pending_action="add"), "New File")
        self.assertEqual(words.status_label(pending_action="edit"), "Ready to Submit")

    def test_untracked_files_read_as_new_even_without_a_pending_action(self):
        self.assertEqual(words.status_label(tracked=False), "New File")

    def test_clean_states_fall_through_to_sync_status(self):
        self.assertEqual(words.status_label(lock_state="self"), "Checked Out")
        self.assertEqual(words.status_label(needs_sync=True), "Needs Sync")
        self.assertEqual(words.status_label(), "Up to Date")

    def test_bridge_lock_and_tracking_vocabulary_is_translated_once(self):
        # The bridge says "mine" and "untracked"; hosts must not each translate.
        mine = SimpleNamespace(
            needs_sync=False, lock_state="mine", pending_action=None, local_state="clean"
        )
        self.assertEqual(words.status_from_bridge(mine), "Checked Out")

        untracked = SimpleNamespace(
            needs_sync=False, lock_state=None, pending_action=None, local_state="untracked"
        )
        self.assertEqual(words.status_from_bridge(untracked), "New File")

        theirs = SimpleNamespace(
            needs_sync=True, lock_state="other", pending_action="edit", local_state="modified"
        )
        self.assertEqual(words.status_from_bridge(theirs), "In Use")
        self.assertEqual(words.status_from_bridge(theirs, blocked=True), "Blocked")

    def test_the_node_copy_agrees_on_a_bridge_status(self):
        script = (
            "const w = require(process.argv[1]);"
            "process.stdout.write(w.statusFromBridge(JSON.parse(process.argv[2])));"
        )
        payload = json.dumps(
            {
                "needs_sync": False,
                "lock_state": "mine",
                "pending_action": None,
                "local_state": "clean",
            }
        )
        result = subprocess.run(
            ["node", "-e", script, str(NODE_WORDS), payload],
            capture_output=True,
            text=True,
            check=True,
        )
        self.assertEqual(result.stdout, "Checked Out")


class ThemeTests(unittest.TestCase):
    def test_palette_matches_the_shared_panel_stylesheet(self):
        # The CSS and the Python palette are two copies of one decision; this is
        # the test that keeps them one decision.
        css = PANEL_CSS.read_text(encoding="utf-8")
        for name, value in theme.PALETTE.items():
            token = f"--oad-{name.replace('_', '-')}: {value};"
            self.assertIn(token, css, f"panel.css is missing {token}")

    def test_colour_conversions_agree(self):
        self.assertEqual(theme.rgb_bytes("primary"), (0x7B, 0xD0, 0xC4))
        floats = theme.rgb_floats("primary")
        self.assertAlmostEqual(floats[0], 0x7B / 255.0)
        self.assertTrue(all(0.0 <= channel <= 1.0 for channel in floats))

    def test_the_unity_overlay_colours_come_from_the_palette(self):
        # Unity draws project-window dots in C#, so its colours are another hand
        # copy. Each `Hex(0xrr, 0xgg, 0xbb)` must name a palette entry.
        source = UNITY_OVERLAY.read_text(encoding="utf-8")
        found = re.findall(r"Hex\(0x(\w\w), 0x(\w\w), 0x(\w\w)\)", source)
        self.assertTrue(found, "no palette colours found in the Unity overlay")
        palette = {value.lstrip("#").lower() for value in theme.PALETTE.values()}
        for channels in found:
            self.assertIn("".join(channels).lower(), palette)

    def test_qt_stylesheet_is_scoped_to_the_panel(self):
        stylesheet = theme.qt_stylesheet()
        self.assertIn("QWidget#OadPanel", stylesheet)
        # Every rule must be scoped, or it would restyle the host application.
        for selector in re.findall(r"(?m)^\s*([^\s{][^{]*)\{", stylesheet):
            self.assertIn("#Oad", selector, f"unscoped selector: {selector.strip()}")


if __name__ == "__main__":
    unittest.main()
