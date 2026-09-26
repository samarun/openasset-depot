# Known Limitations

OpenAsset Depot's core centralized version-control workflow is functional, but a
responsible production rollout should account for these boundaries:

- The object backend is a content-addressed local filesystem by default. S3 is
  available behind `OAD_STORAGE_BACKEND=s3` and the `s3` cargo feature; orphan
  cleanup still walks only the local filesystem.
- Unreferenced immutable chunks created before a failed metadata transaction are
  harmless and can be reported/removed with the admin cleanup endpoint. A
  scheduled integrity worker can run the same verify-and-clean pass when
  `OAD_INTEGRITY_INTERVAL_SECONDS` is set.
- Authorization supports system admin, per-user and per-group depot `read`,
  `write`, and `admin` roles, and optional OIDC. Stream-level ACLs, token
  revocation, and MFA are not implemented.
- JWT is the MVP authentication mechanism. Use HTTPS, short TTLs appropriate to
  the studio, and an external reverse proxy/WAF for IP-aware login throttling.
- Supported clients use keyset lock, audit, and dependency pages. The legacy
  unpaged `/api/locks` response remains available for protocol-v1 compatibility.
- Dependency scanners are rule/text based. The inspector reports who depends on
  a file and what it depends on; recursive dependency sync, reference repair,
  render-farm fanout, and preview generation workers are future pipeline services.
- Review requests approve a specific immutable revision. Stage orchestration is
  not shipped.
- Force-release/admin lock takeover is an admin API with an audit trail, not an
  artist-facing control.
- Adobe uses CEP and requires studio signing for production distribution.
  Resolve Workflow Integrations require Resolve Studio on macOS/Windows.
- Blender host smoke runs on every push. Maya, Houdini, and Nuke smoke runs on
  self-hosted licensed runners when `OAD_LICENSED_HOST_RUNNERS` is set; see
  [host CI](host-ci.md). Unreal 5.5 source was matched to installed headers;
  local binary compilation is blocked by the machine's newer Xcode SDK, which
  UE 5.5 rejects before compiling plug-in code.
