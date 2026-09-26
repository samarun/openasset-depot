# Testing

## Fast release gates

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib
cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --all-targets -- -D warnings

cd desktop
npm ci
npm test -- --run
npm run build

cd ..
./scripts/check-terminology.py
PYTHONPATH=plugins/common python3 -m unittest discover -s plugins/common/tests -v
node --test plugins/common/node/test/openasset-cli.test.js
./scripts/host-smoke.py
./scripts/package-integrations.py
```

The TypeScript SDK is checked with `desktop/node_modules/.bin/tsc -p
sdk/typescript/tsconfig.json --noEmit`. Docker release gates build both API and
web images and validate the Compose model.

## Isolated API integration suite

```sh
OAD_TEST_API_PORT=18082 OAD_TEST_POSTGRES_PORT=55434 \
docker compose -p oad-test -f deploy/docker-compose.test.yml up -d --build

OAD_TEST_BASE_URL=http://127.0.0.1:18082 \
OAD_TEST_DATABASE_URL=postgres://openasset_test:isolated-test-password@127.0.0.1:55434/openasset_test \
cargo test -p openasset-api --test required_flows -- --ignored --test-threads=1

OAD_TEST_API_PORT=18082 OAD_TEST_POSTGRES_PORT=55434 \
docker compose -p oad-test -f deploy/docker-compose.test.yml down -v
```

The disposable stack uses dedicated ports and volumes. The example overrides
avoid common local port collisions; omit them to use API `18081` and PostgreSQL
`55433`. The suite covers
concurrent lock races, case-insensitive lock uniqueness, admin/user permissions,
atomic submit rollback, identical submit retries, duplicate multipart parts,
tombstone delete/sync, incremental sync acknowledgement, large-file streaming,
path traversal, audit events, history, idempotent creates, workspace deletion,
keyset lock/audit pages, persisted dependency scans, admin-only dry-run cleanup,
and object integrity verification.

## Host checks

Python sources and extracted archives are byte-compiled, XML/JSON manifests are
parsed, Node bridges are syntax/unit tested, package checksums are verified, and
`scripts/check-terminology.py` fails on retired wording — all on every push.

`./scripts/host-smoke.py` runs each integration's panel inside the real
application. It skips hosts that are not installed (exit code 77) rather than
failing, so it is safe to run anywhere; `--require-host` turns a missing host
into a failure, which is what the CI legs use. Blender runs on every push against
3.6 and 4.2; Maya, Houdini, and Nuke run on studio-provided self-hosted runners.

[docs/host-ci.md](host-ci.md) covers what those tests assert and how to attach a
licensed runner.
