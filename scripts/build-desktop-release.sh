#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TARGET=${CARGO_BUILD_TARGET:-$(rustc -vV | awk '/^host:/ { print $2 }')}
EXE_SUFFIX=""
case "$TARGET" in
  *-windows-*) EXE_SUFFIX=".exe" ;;
esac

cd "$ROOT"
cargo build --release -p oad --target "$TARGET"
mkdir -p desktop/src-tauri/binaries
cp "target/$TARGET/release/oad$EXE_SUFFIX" "desktop/src-tauri/binaries/oad-$TARGET$EXE_SUFFIX"

cd desktop
npm ci
npm test -- --run

case "$TARGET" in
  *-apple-darwin)
    if [ -n "${APPLE_CERTIFICATE:-}" ]; then
      npm run tauri build -- --target "$TARGET"
    else
      SIGNING_IDENTITY=${OAD_MACOS_SIGNING_IDENTITY:--}
      npm run tauri build -- --target "$TARGET" \
        --config "{\"bundle\":{\"macOS\":{\"signingIdentity\":\"$SIGNING_IDENTITY\"}}}"
    fi
    codesign --verify --deep --strict \
      "src-tauri/target/$TARGET/release/bundle/macos/OpenAsset Depot.app"
    ;;
  *)
    npm run tauri build -- --target "$TARGET"
    ;;
esac
