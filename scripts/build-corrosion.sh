#!/usr/bin/env bash
# Build the exact peer protocol shipped with this Tilde release.
set -euo pipefail
root_dir="$(cd "$(dirname "$0")/.." && pwd)"
revision=753cfd2408d67edeaf3052a990175356ed4dcbc6
source_dir="$root_dir/.tools/corrosion-src"
output_dir="$root_dir/.tools/corrosion-target"
mkdir -p "$root_dir/.tools"
if [[ -x "$root_dir/.tools/corrosion" && -f "$root_dir/.tools/corrosion.revision" ]] && [[ "$(cat "$root_dir/.tools/corrosion.revision")" == "$revision" ]]; then exit 0; fi
if [[ ! -d "$source_dir/.git" ]]; then git clone --filter=blob:none --no-checkout https://github.com/superfly/corrosion.git "$source_dir"; fi
git -C "$source_dir" fetch --depth 1 origin "$revision"
git -C "$source_dir" checkout --detach "$revision"
(cd "$source_dir" && CARGO_TARGET_DIR="$output_dir" cargo +1.98.1 build --locked --release -p corrosion)
cp "$output_dir/release/corrosion" "$root_dir/.tools/corrosion"
if command -v strip >/dev/null; then strip "$root_dir/.tools/corrosion"; fi
cp "$source_dir/LICENSE" "$root_dir/.tools/corrosion.LICENSE"
printf '%s\n' "$revision" > "$root_dir/.tools/corrosion.revision"
