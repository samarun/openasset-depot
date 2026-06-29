# Host Integrations

This directory contains working source integrations for Blender, Maya, Houdini,
Nuke, Unreal Engine, Unity, Adobe Creative Cloud, and DaVinci Resolve Studio.
`common/` owns the dependency-free Python and Node CLI bridges.

Build self-contained archives with:

```sh
./scripts/package-integrations.py
```

The production boundary is deliberate: plug-ins discover the current scene or
selected assets and invoke the versioned `oad integration` protocol. Locking,
permissions, validation, immutable storage, submit transactions, and audit remain
authoritative in the Rust server. See `docs/integrations.md` for installation and
host verification status.
