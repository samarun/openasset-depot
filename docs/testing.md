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
PYTHONPATH=plugins/common python3 -m unittest discover -s plugins/common/tests -v
node --test plugins/common/node/test/openasset-cli.test.js
./scripts/package-integrations.py
```

The TypeScript SDK is checked with `desktop/node_modules/.bin/tsc -p
sdk/typescript/tsconfig.json --noEmit`. Docker release gates build both API and
web images and validate the Compose model.

## Isolated API integration suite

```sh
docker compose -p oad-test -f deploy/docker-compose.test.yml up -d --build

OAD_TEST_BASE_URL=http://127.0.0.1:18081 \
OAD_TEST_DATABASE_URL=postgres://openasset_test:isolated-test-password@127.0.0.1:55433/openasset_test \
cargo test -p openasset-api --test required_flows -- --ignored --test-threads=1

docker compose -p oad-test -f deploy/docker-compose.test.yml down -v
```

The disposable stack uses dedicated ports and volumes. The suite covers
concurrent lock races, case-insensitive lock uniqueness, admin/user permissions,
atomic submit rollback, identical submit retries, duplicate multipart parts,
tombstone delete/sync, incremental sync acknowledgement, large-file streaming,
path traversal, audit events, history, idempotent creates, workspace deletion,
keyset lock/audit pages, persisted dependency scans, admin-only dry-run cleanup,
and object integrity verification.

## Host checks

Python sources and extracted archives are byte-compiled, XML/JSON manifests are
parsed, Node bridges are syntax/unit tested, and package checksums are verified in
CI. Blender registration/teardown is host-tested when Blender is available.
Licensed host CI images should additionally open a fixture project, run status,
checkout, save, submit, sync from a second workspace, and unload the plug-in.
