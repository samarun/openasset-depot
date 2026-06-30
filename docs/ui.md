# Desktop UI

The first OpenAsset Depot UI lives in `desktop/`.

It is designed as a premium production tool for artists, TDs, engineers, and virtual-production teams. It keeps Perforce-class workflow concepts but uses a calmer hierarchy, larger touch targets, clear status language, progressive disclosure, and a command palette.

## Screens

- Login
- Workspace selector
- Home dashboard
- Workspace file browser
- File inspector
- Changelist panel
- Submit dialog
- Lock Center
- File history
- Settings
- Admin

Reviews and Stage pages are planned for a future release and are hidden from production navigation until their backend APIs are implemented.

## Component Structure

- `StatusBadge`
- `FileRow`
- `AssetCard`
- `ChangelistCard`
- `LockCard`
- `FileInspector`
- `SubmitChecklist`
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

- **Artist:** My Work, Assets, Changes, Locks
- **Lead/TD:** + History
- **Admin:** + Admin, Settings

Admin-only sections are hidden from non-admin users.

Fresh administrators can create the first depot, main stream, and local
workspace from one onboarding flow. This avoids requiring an existing workspace
before the Admin surface becomes reachable.

## Long-running operations

The desktop uses additive integration protocol v2 for sync and submit. The CLI
emits newline-delimited progress and result messages, Tauri forwards progress as
scoped window events, and the UI shows the current phase. Existing DCC clients
remain on the default v1 single-result envelope.

The browser uses a separate web-intake workflow: selected `File` objects are
validated and sent as multipart changelist content. Browser refresh reads depot
state; it never claims to write an arbitrary local project folder. Desktop and
DCC workspaces consume browser submissions through normal verified sync.

## Studio Design Language

The desktop experience is organized around active production work rather than management metrics. My Work opens with project context, a continuous production-status rail, the asset workbench, a personal action queue, and depot activity. A graphite production rail separates creative workflow from administrative utilities, while teal, amber, blue, and coral are reserved for source-control meaning.
