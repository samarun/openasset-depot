# Known Limitations

OpenAsset Depot's core centralized version-control workflow is functional, but a
responsible production rollout should account for these boundaries:

- The object backend is a content-addressed local filesystem volume. It supports
  large streamed binaries and backup/restore, but not multi-node API writers or
  S3-compatible storage yet. Run one API writer against one durable object volume.
- Unreferenced immutable chunks created before a failed metadata transaction are
  harmless and can be reported/removed with the admin cleanup endpoint. Cleanup
  is manual rather than scheduled. Request temporaries are removed immediately
  and crash leftovers older than 24 hours are cleaned at startup.
- Authorization supports system admin plus per-user depot `read`, `write`, and
  `admin` roles. Group, SSO/OIDC, service accounts, stream-level ACLs, token
  revocation, and MFA are not implemented.
- JWT is the MVP authentication mechanism. Use HTTPS, short TTLs appropriate to
  the studio, and an external reverse proxy/WAF for IP-aware login throttling.
- Supported clients use keyset lock, audit, and dependency pages. The legacy
  unpaged `/api/locks` response remains available for protocol-v1 compatibility.
- Dependency scanners are rule/text based. Recursive dependency sync, reference
  repair, impact analysis, render-farm fanout, and preview generation workers are
  future pipeline services.
- Reviews and Stage views remain presentation surfaces; no review approval or
  stage orchestration backend is shipped in this release.
- Force-release/admin lock takeover is intentionally not exposed in the artist UI.
- Adobe uses CEP and requires studio signing for production distribution.
  Resolve Workflow Integrations require Resolve Studio on macOS/Windows.
- Blender 5.1 host registration is verified locally. Maya, Houdini, Nuke, Unity,
  Adobe, and Resolve source/package validation passes, but their proprietary host
  automation suites require licensed CI images. Unreal 5.5 source was matched to
  installed headers; local binary compilation is blocked by the machine's newer
  Xcode 26.4 SDK, which UE 5.5 rejects before compiling plug-in code.
