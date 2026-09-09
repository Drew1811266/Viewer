#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
OUTPUT_ROOT=${VIEWER_IMAGE_RENDER_OUTPUT_DIR:-"$REPO_ROOT/target/image-render-performance"}
RECEIPT="$OUTPUT_ROOT/receipt.json"
MAGNIFIER_RECEIPT="$OUTPUT_ROOT/receipt-magnifier.json"
SAVE_LATENCY="$OUTPUT_ROOT/save-latency.ndjson"

mkdir -p "$OUTPUT_ROOT"

PERFORMANCE_FIXTURE_ROOT="$OUTPUT_ROOT/fixture"
if [ -n "${VIEWER_IMAGE_RENDER_FIXTURE_ROOT:-}" ]; then
  SAVE_FIXTURE_ROOT=$VIEWER_IMAGE_RENDER_FIXTURE_ROOT
  if [ ! -d "$SAVE_FIXTURE_ROOT" ]; then
    echo "VIEWER_IMAGE_RENDER_FIXTURE_ROOT is not a directory" >&2
    exit 2
  fi
else
  SAVE_FIXTURE_ROOT=$PERFORMANCE_FIXTURE_ROOT
fi

# Keep the fixed GPU workload deterministic even when save testing uses an
# existing user corpus. Never generate a fixture in the user's source folder.
mkdir -p "$PERFORMANCE_FIXTURE_ROOT"
swift "$SCRIPT_DIR/generate-performance-fixtures.swift" \
  --output "$PERFORMANCE_FIXTURE_ROOT/viewer-native-8k.jpg" >/dev/null

cargo run --locked --release -p viewer-desktop --example image_render_acceptance -- \
  "$PERFORMANCE_FIXTURE_ROOT/viewer-native-8k.jpg" "$RECEIPT" main

cargo run --locked --release -p viewer-desktop --example image_render_acceptance -- \
  "$PERFORMANCE_FIXTURE_ROOT/viewer-native-8k.jpg" "$MAGNIFIER_RECEIPT" magnifier

cargo run --locked -p viewer-desktop --example review_save_latency -- \
  --project "$SAVE_FIXTURE_ROOT" --samples 30 >"$SAVE_LATENCY"

node "$SCRIPT_DIR/performance-gate.mjs" \
  --receipt "$RECEIPT" \
  --save-latency "$SAVE_LATENCY" --scenario main

node "$SCRIPT_DIR/performance-gate.mjs" \
  --receipt "$MAGNIFIER_RECEIPT" \
  --save-latency "$SAVE_LATENCY" --scenario magnifier

echo "$RECEIPT"
echo "$MAGNIFIER_RECEIPT"
