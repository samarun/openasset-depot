"""Canonical user-facing wording shared by every host integration.

Nine host panels drifted into saying different things for the same operation:
"Revert Checkout" versus "Revert Intent", "Add Current Scene" versus "Add to
Depot", bare "Refresh" versus "Refresh Status". An artist who moves between Maya
and Nuke in one afternoon should not have to relearn the vocabulary, so every
panel now composes its labels from this module instead of hard-coding strings.

The wording here matches the desktop app and ``docs/glossary.md``. Changing a
term means changing it in one place, and ``scripts/check-terminology.py`` fails
CI if a plugin reintroduces a retired synonym.

Host qualifiers are expected and allowed: Houdini legitimately distinguishes the
HIP file from a selected HDA. Compose those with :func:`qualified`, which keeps
the canonical verb at the front where it is scannable.
"""

from __future__ import annotations

from types import MappingProxyType
from typing import Mapping

PRODUCT_NAME = "OpenAsset Depot"
"""Full product name. Panels title-bar this; never abbreviate it to "OpenAsset"."""

SHORT_NAME = "Depot"
"""Space-constrained label, for example a Blender sidebar tab."""

ACTIONS: Mapping[str, str] = MappingProxyType(
    {
        "refresh": "Refresh Status",
        "checkout": "Check Out",
        "add": "Add to Depot",
        "sync": "Sync Latest",
        "validate": "Validate",
        "submit": "Submit Changes",
        # "Revert Intent" rather than "Revert Checkout" because this operation
        # drops the pending source-control operation and releases the lock while
        # leaving the artist's local file untouched. "Revert" alone reads, to
        # anyone arriving from git or Perforce, as "throw away my work".
        "revert": "Revert Intent",
        "unlock": "Release Lock",
        "shelve": "Shelve Changes",
        "unshelve": "Restore Shelf",
        "history": "View History",
    }
)
"""Button and menu labels, keyed by the operation the bridge exposes."""

STATUSES: Mapping[str, str] = MappingProxyType(
    {
        "up_to_date": "Up to Date",
        "needs_sync": "Needs Sync",
        "checked_out": "Checked Out",
        "in_use": "In Use",
        "ready_to_submit": "Ready to Submit",
        "blocked": "Blocked",
        "marked_for_delete": "Marked for Delete",
        "new_file": "New File",
    }
)
"""The eight artist-facing statuses, identical to the desktop app's ``StatusKind``."""

PROGRESS: Mapping[str, str] = MappingProxyType(
    {
        "refresh": "Refreshing status",
        "checkout": "Checking out",
        "add": "Adding to depot",
        "sync": "Downloading files",
        "validate": "Validating",
        "submit": "Submitting changes",
        "revert": "Reverting intent",
        "shelve": "Shelving changes",
        "unshelve": "Restoring shelf",
        "preview": "Uploading preview",
    }
)
"""Present-participle phrases for in-flight operations, so progress text matches."""

FIELDS: Mapping[str, str] = MappingProxyType(
    {
        "description": "Description",
        "reason": "Reason",
        "server": "Server",
        "workspace": "Workspace",
    }
)
"""Form field labels. The per-host "Submit note" variants are retired."""

MESSAGES: Mapping[str, str] = MappingProxyType(
    {
        "no_workspace": (
            f"This project is not inside an {PRODUCT_NAME} workspace. "
            "Create one in the desktop app, then reopen the file from there."
        ),
        "not_signed_in": (
            f"Sign in to {PRODUCT_NAME} in the desktop app or with "
            "`oad login`, then try again."
        ),
        "save_first": "Save the file before running this action.",
        "locked_by_other": (
            "Another artist has this checked out. Ask them to submit or release "
            "the lock, then try again."
        ),
        "nothing_pending": "There are no pending changes to submit.",
        "nothing_to_shelve": "There are no pending changes to shelve.",
        "need_description": (
            f"Enter a {FIELDS['description'].lower()} before submitting."
        ),
        "needs_sync_first": (
            f"A newer revision exists on the server. Run {ACTIONS['sync']} "
            "before submitting."
        ),
    }
)
"""Recurring explanations, phrased to say what the artist should do next."""

# Wording that used to appear in one host or another. The terminology check
# treats each key as banned and points at the replacement, which is what makes
# the drift this module fixes stay fixed.
#
# Entries are label-shaped and matched case-sensitively on purpose. A rule broad
# enough to also catch explanatory prose ("ask the artist who has it checked
# out") would either be ignored or force worse sentences, and the problem being
# solved is inconsistent *labels*, not inconsistent prose.
RETIRED_TERMS: Mapping[str, str] = MappingProxyType(
    {
        "Revert Checkout": ACTIONS["revert"],
        "Revert HIP Checkout": ACTIONS["revert"],
        "Add Current Scene": ACTIONS["add"],
        "Add Asset + .meta": ACTIONS["add"],
        "Validate Scene": ACTIONS["validate"],
        "Validate Script": ACTIONS["validate"],
        "Validate Asset": ACTIONS["validate"],
        "Submit note": FIELDS["description"],
        "Submit description": FIELDS["description"],
        "Submit Description": FIELDS["description"],
        "New version available": STATUSES["needs_sync"],
        "New versions available": STATUSES["needs_sync"],
        "Changes ready to submit": STATUSES["ready_to_submit"],
    }
)


def qualified(action: str, target: str) -> str:
    """Builds a host-specific label such as ``"Check Out Selected HDA"``.

    Keeps the canonical verb first so the button still scans as the same action
    across hosts, with the host's noun appended rather than substituted.
    """

    verb = ACTIONS[action]
    target = target.strip()
    return f"{verb} {target}" if target else verb


def progress_for(operation: str, target: str | None = None) -> str:
    """Builds progress text, optionally naming what is being worked on."""

    phrase = PROGRESS.get(operation, operation.replace("_", " ").capitalize())
    return f"{phrase} {target}" if target else phrase


def status_label(
    *,
    needs_sync: bool = False,
    lock_state: str | None = None,
    pending_action: str | None = None,
    tracked: bool = True,
    blocked: bool = False,
) -> str:
    """Maps a bridge ``FileStatus`` onto one canonical status label.

    Order matters and is the same ordering the desktop app uses: a validation
    problem outranks everything because it is the thing stopping a submit, and
    another artist's lock outranks the caller's own pending edit because it is
    the thing they cannot resolve alone.
    """

    if blocked:
        return STATUSES["blocked"]
    if lock_state == "other":
        return STATUSES["in_use"]
    if pending_action == "delete":
        return STATUSES["marked_for_delete"]
    if pending_action == "add" or not tracked:
        return STATUSES["new_file"]
    if pending_action:
        return STATUSES["ready_to_submit"]
    if lock_state == "self":
        return STATUSES["checked_out"]
    if needs_sync:
        return STATUSES["needs_sync"]
    return STATUSES["up_to_date"]


def status_from_bridge(status, *, blocked: bool = False) -> str:
    """Labels a bridge ``FileStatus`` without each host re-deriving the rules.

    The bridge reports locks as ``"mine"``/``"other"`` and trackedness as a
    ``local_state`` of ``"untracked"``. Translating that in one place is the
    point: four hosts previously each wrote their own version, which is how
    Maya ended up saying "Checked Out by Me" while Nuke said "Checked Out".
    """

    return status_label(
        needs_sync=bool(status.needs_sync),
        lock_state="self" if status.lock_state == "mine" else status.lock_state,
        pending_action=status.pending_action,
        tracked=status.local_state != "untracked",
        blocked=blocked,
    )
