#!/bin/sh
set -eu

SOURCE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
DESTINATION=${1:-"$HOME/Library/Application Support/Adobe/CEP/extensions/com.openassetdepot.creativecloud"}

mkdir -p "$DESTINATION/lib"
cp -R "$SOURCE_DIR/CSXS" "$DESTINATION/"
cp -R "$SOURCE_DIR/js" "$DESTINATION/"
cp -R "$SOURCE_DIR/jsx" "$DESTINATION/"
cp "$SOURCE_DIR/index.html" "$DESTINATION/index.html"
cp "$SOURCE_DIR/../../common/node/openasset-cli.js" "$DESTINATION/lib/openasset-cli.js"
cp "$SOURCE_DIR/../../common/node/words.js" "$DESTINATION/lib/words.js"
cp "$SOURCE_DIR/../../common/visual-kit/panel.css" "$DESTINATION/lib/panel.css"

printf 'Installed OpenAsset Depot CEP panel at %s\n' "$DESTINATION"
printf 'Restart the Adobe host, then open Window > Extensions (Legacy) > OpenAsset Depot.\n'
