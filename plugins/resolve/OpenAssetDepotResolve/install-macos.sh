#!/bin/sh
set -eu

SOURCE_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
DESTINATION=${1:-"/Library/Application Support/Blackmagic Design/DaVinci Resolve/Workflow Integration Plugins/com.openassetdepot.resolve"}
SDK_NODE=${RESOLVE_WORKFLOW_NODE:-"/Library/Application Support/Blackmagic Design/DaVinci Resolve/Developer/Workflow Integrations/Examples/SamplePlugin/WorkflowIntegration.node"}

if [ ! -f "$SDK_NODE" ]; then
  printf 'Resolve WorkflowIntegration.node was not found at %s\n' "$SDK_NODE" >&2
  exit 1
fi

mkdir -p "$DESTINATION/lib"
cp "$SOURCE_DIR/manifest.xml" "$DESTINATION/manifest.xml"
cp "$SOURCE_DIR/package.json" "$DESTINATION/package.json"
cp "$SOURCE_DIR/main.js" "$DESTINATION/main.js"
cp "$SOURCE_DIR/preload.js" "$DESTINATION/preload.js"
cp "$SOURCE_DIR/index.html" "$DESTINATION/index.html"
cp "$SOURCE_DIR/renderer.js" "$DESTINATION/renderer.js"
cp "$SOURCE_DIR/styles.css" "$DESTINATION/styles.css"
cp "$SDK_NODE" "$DESTINATION/WorkflowIntegration.node"
cp "$SOURCE_DIR/../../common/node/openasset-cli.js" "$DESTINATION/lib/openasset-cli.js"

printf 'Installed OpenAsset Depot Resolve integration at %s\n' "$DESTINATION"
printf 'Restart Resolve Studio, then open Workspace > Workflow Integrations > OpenAsset Depot.\n'
