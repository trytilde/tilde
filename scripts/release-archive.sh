#!/usr/bin/env bash
set -euo pipefail
version="$(cat VERSION)"
target="${RELEASE_TARGET:?Set RELEASE_TARGET to the native Rust target triple}"
test "$(rustc -vV | sed -n 's/^host: //p')" = "$target"
binary_dir="${CARGO_TARGET_DIR:-target}/release"
test "$("$binary_dir/tilde" --version)" = "tilde $version"
mkdir -p dist/release
archive="tilde-v${version}-${target}.tar.gz"
tar -C "$binary_dir" -czf "dist/release/$archive" tilde
(
  cd dist/release
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$archive" > "$archive.sha256"
  else
    shasum -a 256 "$archive" > "$archive.sha256"
  fi
)
