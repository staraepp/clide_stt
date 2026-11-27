#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
metal_dir="$root/src-tauri/vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-metal"
out="$root/src-tauri/resources/default.metallib"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

cp "$root/src-tauri/vendor/whisper-rs-sys/whisper.cpp/ggml/src/ggml-common.h" "$tmp/"
cp "$metal_dir/ggml-metal.metal" "$metal_dir/ggml-metal-impl.h" "$tmp/"

xcrun -sdk macosx metal -O3 -c "$tmp/ggml-metal.metal" -o "$tmp/ggml-metal.air"
xcrun -sdk macosx metallib "$tmp/ggml-metal.air" -o "$out"

echo "Wrote $out ($(du -h "$out" | cut -f1))"
