# Glossary

The canonical vocabulary for every surface of OpenAsset Depot: the desktop app,
the `oad` CLI, and all nine host integrations.

This document exists because the same operation used to be called three
different things depending on where an artist happened to be standing. Maya said
"Revert Checkout", Blender said "Revert Intent", Unity said "Revert Checkout"
next to an "Add Asset + .meta" button that no other host had. An artist who
moves between Maya and Nuke in one afternoon should not have to relearn the
vocabulary.

## How the wording is enforced

The labels below are not documentation of what the code happens to say; they are
the source the code reads from.

| Surface | Source of the wording |
| --- | --- |
| Maya, Houdini, Nuke, Blender | `plugins/common/openasset_depot_bridge/words.py` |
| Adobe, DaVinci Resolve | `plugins/common/node/words.js` |
| Unity | `plugins/unity/OpenAssetDepotUnity/Editor/OpenAssetWords.cs` |
| Desktop app | `desktop/src/types/domain.ts` (`StatusKind`) and its components |

`scripts/check-terminology.py` runs in CI and fails when a retired synonym
reappears in a plugin, the desktop app, or these docs. The Node and C# copies
exist only because those languages cannot import the Python module;
`plugins/common/tests/test_words.py` fails if any copy drifts.

Changing a term means changing `words.py`, then the two copies, then this file.

## Actions

What an artist can ask the system to do. Hosts may append a noun when they
genuinely act on more than one kind of thing — Houdini distinguishes the HIP
file from a selected HDA, so it shows **Check Out HIP** and **Check Out Selected
HDA** — but the verb always comes first and is never substituted.

| Label | Meaning |
| --- | --- |
| **Refresh Status** | Re-read this file's lock, sync, and pending state from the server. Changes nothing. |
| **Check Out** | Take the exclusive lock and mark the file for edit. Required before editing a binary asset. |
| **Add to Depot** | Mark a file the depot has never seen for its first submission. |
| **Sync Latest** | Download newer revisions from the server. Never overwrites a file you have checked out. |
| **Validate** | Run the depot and host-adapter checks that must pass before a submit. |
| **Submit Changes** | Publish every pending file in the workspace as new immutable revisions and release their locks. |
| **Revert Intent** | Drop the pending source-control operation and release the lock, leaving the local file exactly as it is. |
| **Release Lock** | Give up a lock without dropping the pending operation. Used by leads to clear a lock somebody else is holding. |
| **Shelve Changes** | Park pending work on the server without creating a revision. |
| **Restore Shelf** | Bring a shelf's files back as pending changes. |
| **View History** | Show the revision list for one file. |

### Why "Revert Intent" and not "Revert"

This is the term that caused the most drift, so it is worth being explicit.

The operation removes the pending source-control operation — the *intent* to
edit, add, or delete — and releases the lock. It does not touch the artist's
local file. Anyone arriving from git or Perforce reads a bare "Revert" as "throw
away my work", which is the one thing this command does not do. "Revert
Checkout" is closer but wrong in the other direction: the command also cancels a
pending add or delete, neither of which involved a checkout.

## Statuses

Exactly eight, matching the desktop app's `StatusKind` union. A plugin inventing
a ninth would be inventing a state the product does not have.

| Label | Meaning |
| --- | --- |
| **Up to Date** | Matches the newest revision on the server, nothing pending. |
| **Needs Sync** | A newer revision exists on the server. |
| **Checked Out** | You hold the lock. |
| **In Use** | Another artist holds the lock. |
| **Ready to Submit** | You have a pending edit waiting for a submit. |
| **Blocked** | Validation failed; the submit cannot proceed. |
| **Marked for Delete** | Pending deletion at the next submit. |
| **New File** | Not yet tracked by the depot, or pending its first add. |

When more than one applies, the most urgent wins, in this order: **Blocked**,
**In Use**, **Marked for Delete**, **New File**, **Ready to Submit**, **Checked
Out**, **Needs Sync**, **Up to Date**. The reasoning is that a validation
failure is what stops a submit, and another artist's lock outranks your own
pending edit because it is the part you cannot resolve alone.

`words.status_from_bridge()` and its JavaScript twin apply this precedence, so a
host should never re-derive it.

## Nouns

| Term | Meaning |
| --- | --- |
| **Depot** | A storage root with its own permissions. Also the short product name where space is tight. |
| **Stream** | A branch of a depot. Workspaces sync one stream. |
| **Workspace** | One artist's local checkout of one stream. |
| **Changelist** | The set of pending operations that a submit publishes together. |
| **Revision** | An immutable published version of one file. |
| **Shelf** | Pending work parked on the server, belonging to one changelist. |
| **Lock** | Exclusive right to edit one file, held by one workspace. |

## Retired synonyms

These fail CI. The replacement is what the product says now.

| Retired | Use instead |
| --- | --- |
| Revert Checkout, Revert HIP Checkout | Revert Intent |
| Add Current Scene, Add Asset + .meta | Add to Depot |
| Validate Scene, Validate Script, Validate Asset | Validate |
| Submit note, Submit description, Submit Description | Description |
| New version available, New versions available | Needs Sync |
| Changes ready to submit | Ready to Submit |

Prose is not policed. "Ask the artist who has it checked out" is a sentence, not
a label, and forcing canonical capitalisation into it would only make the
sentence worse. The check matches case-sensitively for that reason.
