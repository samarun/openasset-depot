# DCC Adapter Architecture

OpenAsset Depot keeps the core server application-agnostic. DCC behavior lives in adapters and SDK/native plugins.

## Layers

- Core backend API: auth, depots, streams, workspaces, locks, changelists, submit, sync, storage, audit.
- SDK layer: Rust and TypeScript contracts for clients and native app bridges.
- Adapter registry: built-in MVP registry for Unreal Engine, Unity, Blender, Maya, Houdini, Nuke, Adobe Premiere, After Effects, Photoshop, DaVinci Resolve, and Generic Media.
- File type rules: database-backed path classification and lock/generated-cache policy.
- Dependency scanner interface: rule-based MVP text/path scanning.
- Preview generator interface: strategy contract only in MVP.
- Validation interface: submit-readiness warnings/errors by adapter.
- Host integrations: source-control panels/providers under `plugins/`, all using
  protocol version 1 through shared Python/Node or native process bridges.

## Adapter Contract

Each adapter defines:

- `app_name`
- `supported_file_types`
- `project_detection_rules`
- `scene_file_patterns`
- `asset_file_patterns`
- `lock_required_patterns`
- `ignored_patterns`
- `generated_cache_patterns`
- `dependency_scan_strategy`
- `preview_generation_strategy`
- `validation_rules`
- `metadata_extraction_rules`

Runtime operations:

- `detect_project(path)`
- `detect_open_file(path)`
- `scan_dependencies(file)`
- `generate_preview(file)`
- `validate_before_submit(changelist)`
- `extract_metadata(file)`
- `get_lock_required_patterns()`
- `get_generated_cache_patterns()`
- `get_ignored_patterns()`

## REST API

- `GET /api/adapters`
- `POST /api/adapters/detect`
- `POST /api/adapters/scan`
- `POST /api/adapters/preview`
- `POST /api/adapters/metadata`
- `POST /api/adapters/validate`

Existing `/api/validate` remains backward compatible and now includes adapter warnings.

## Dependency Graph

The universal graph is directed:

- `asset_nodes`: stream/path nodes with adapter metadata.
- `asset_dependency_edges`: source-to-target edges with `dependency_type`, `dependency_status`, `adapter_name`, `scan_time`, and `confidence`.

This supports future queries such as:

- What does this file depend on?
- What uses this file?
- Which scenes use this texture?
- Which Unreal maps reference this material?
- Which Unity scenes use this prefab?
- Which Maya shots use this rig?
- Which Nuke scripts reference this plate?

## Built-in scanning

- Unity: detects required `.meta` companion files.
- Maya: scans simple `.ma` text references.
- Nuke: scans simple Read/Write-like paths and flags absolute/external paths.
- Unreal, Blender, Houdini, Adobe, and Resolve: host integrations provide the
  active scene/selection boundary; binary dependency extraction remains an
  external worker strategy.

Core adapter tests remain host-independent. Package tests run without proprietary
licenses; Blender registration is additionally verified when Blender is
available. Licensed studio CI should run fixture projects in every purchased host.
