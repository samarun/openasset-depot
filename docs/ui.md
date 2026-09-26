# Desktop UI

The first OpenAsset Depot UI lives in `desktop/`.

It is designed as a premium production tool for artists, TDs, engineers, and virtual-production teams. It keeps Perforce-class workflow concepts but uses a calmer hierarchy, larger touch targets, clear status language, progressive disclosure, and a command palette.

## Screens

- Login
- Workspace selector
- Home dashboard
- Workspace file browser
- File inspector, including dependency impact
- Changelist panel
- Submit dialog
- Shelves
- Reviews (sign-off requests)
- Lock Center
- File history
- Settings
- Admin

Stage orchestration remains a future surface and is not in production navigation.

## Component Structure

- `StatusBadge`
- `FileRow`
- `AssetCard`
- `ChangelistCard`
- `LockCard`
- `FileInspector`
- `SubmitChecklist`
- `ReviewRequestDialog`
- `OperationProgressBar`
- `CommandPalette`
- `Sidebar`
- `TopBar`
- `EmptyState`
- `ConfirmDialog`
- `ConnectionStatusBar`
- `ActionNotice`

## Data Honesty

Connected sessions use only backend data. Workspace assets are derived from the server sync plan, and lock, unlock, file operation, validation, and history actions call the REST API directly.

If the server cannot be reached, the app switches to an explicit Demo Mode. A compact connection bar remains visible with retry and settings actions. Demo fixtures are never mixed into a connected session, and unsupported connected surfaces use action-based empty states instead.

## Status Vocabulary

The UI uses a consolidated set of artist-facing statuses that map directly to server and CLI truth. Every badge the user sees corresponds to a real backend state.

| Status | Meaning |
|--------|---------|
| Up to Date | Synced to the latest depot revision |
| Needs Sync | A newer revision exists on the server |
| Checked Out | The current user holds an exclusive lock |
| In Use | Another user holds an exclusive lock |
| Ready to Submit | Pending local changes that can be submitted |
| Blocked | A validation error prevents submission |
| New File | A locally added file not yet in the depot |
| Marked for Delete | File is scheduled for deletion on next submit |

Status mapping is centralized in `desktop/src/components/StatusBadge.tsx` and `desktop/src/data/assets.ts`.

## Connection States

The sidebar status indicator reflects three truthful states derived from the actual connection mode:

| State | Visual | Meaning |
|-------|--------|---------|
| Connected | Green dot | Server is reachable and session is live |
| Reconnecting | Pulsing amber dot | Attempting to reach the server |
| Offline | Grey dot | Running in demo mode |

## Role-Based Navigation

Navigation is organized by role to reduce noise:

- **Artist:** My Work, Assets, Changes, Shelves, Reviews, Locks
- **Lead/TD:** + History
- **Admin:** + Admin, Settings

Admin-only sections are hidden from non-admin users.

Fresh administrators can create the first depot, main stream, and local
workspace from one onboarding flow. This avoids requiring an existing workspace
before the Admin surface becomes reachable.

## First-Run Guide

A newly linked workspace shows `FirstSyncGuide` on My Work: sync the depot, open
the folder in a creative app, then check out before editing. Only the first step
is a control; the rest describe what happens next rather than offering buttons
that cannot work yet.

The guide retires as soon as a sync completes, even when that sync transferred
nothing, so a studio whose depot is still empty is not told to sync forever.
Completion is recorded per workspace id in `localStorage`, and it never appears
in Demo Mode because there is no real folder to sync into.

## Long-running operations

The desktop uses additive integration protocol v2 for sync and submit. The CLI
emits newline-delimited progress and result messages, Tauri forwards progress as
scoped window events, and the UI shows the current phase. Existing DCC clients
remain on the default v1 single-result envelope.

The browser uses a separate web-intake workflow: selected `File` objects are
validated and sent as multipart changelist content. Browser refresh reads depot
state; it never claims to write an arbitrary local project folder. Desktop and
DCC workspaces consume browser submissions through normal verified sync.

## Design Tokens and Motion

Visual primitives live in `desktop/src/design-tokens.css`, imported before
`styles.css` from `desktop/src/main.tsx`. Colour, spacing (`--space-1` … `--space-6`),
radii, elevation, type, and motion are all declared there, with a dark theme
override under `:root[data-theme="dark"]`.

`styles.css` contains no literal `font-size` values. Every size resolves through
the type scale below, so a heading cannot drift on a single page:

| Token | Size | Used for |
|-------|------|----------|
| `--text-display` | 34px | Page and hero titles |
| `--text-display-sm` | 26px | Compact display, narrow viewports |
| `--text-title` | 19px | Section, dialog, and panel titles |
| `--text-lead` | 15px | Prominent body: asset names, lead paragraphs |
| `--text-body` | 13px | Default interface text |
| `--text-label` | 12px | Dense chrome and field labels |
| `--text-caption` | 11px | Metadata and badges |
| `--text-micro` | 10px | Eyebrows and counters |

Headings may only use the top three steps. Weights are limited to
`--weight-regular` and `--weight-strong`. Adding a step is a deliberate design
change, not a per-page adjustment: an earlier revision of this file had grown to
eighteen distinct sizes, including four competing heading sizes between 34px and
46px.

Motion uses three durations — `--motion-fast` (120ms, hover), `--motion-standard`
(180ms), and `--motion-slow` (240ms, state changes such as an acquired lock or a
successful submit) — paired with `--ease-standard` and `--ease-emphasized`.
Because every transition references a token, the `prefers-reduced-motion` block
collapses all three durations to `1ms` and disables animation app-wide in one
place.

## Generated Asset Previews

Assets without a server-rendered thumbnail get deterministic generated art
rather than a flat colour tile. `desktop/src/data/previewSignature.ts` derives a
hue, an accent hue, a rotation, and one of four geometric patterns from the
depot path and file extension. The path drives the hue so each asset is
distinguishable; the extension drives the pattern so a file type reads as a
family. The result is intentionally abstract and must never resemble a render of
the real asset — an honest placeholder is preferable to a convincing but false
one. Real thumbnails replace it as soon as the preview API returns one.

## Visual Regression Baseline

`desktop/src/test/visual-regression.test.tsx` snapshots the rendered markup of
My Work, the asset browser, and the submit sheet using the deterministic demo
fixtures. These run in jsdom as part of `npm test`, so they assert structure and
class names rather than pixels. Update them deliberately with `npx vitest -u`
after an intentional restyle.

## Studio Design Language

The desktop experience is organized around active production work rather than management metrics. My Work opens with project context, a continuous production-status rail, the asset workbench, a personal action queue, and depot activity. A graphite production rail separates creative workflow from administrative utilities, while teal, amber, blue, and coral are reserved for source-control meaning.
