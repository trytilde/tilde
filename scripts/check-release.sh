#!/usr/bin/env bash
set -euo pipefail
python3 scripts/check-versions.py
version="$(cat VERSION)"
# Only stable releases are published to npm's latest and GHCR's latest tags.
if [[ ! "$version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  echo 'Release publishing currently requires a stable semantic version.' >&2
  exit 1
fi
tag="v$version"
test "$(changie latest)" = "$tag"
changie merge
git diff --exit-code -- CHANGELOG.md VERSION Cargo.toml sdk/ts/packages
if git show-ref --verify --quiet "refs/tags/$tag"; then
  test "$(git rev-list -n 1 "$tag")" = "$(git rev-parse HEAD)" || {
    echo "$tag already points at another commit; prepare a new version." >&2
    exit 1
  }
fi
if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  echo "tag=$tag" >> "$GITHUB_OUTPUT"
  echo "version=$version" >> "$GITHUB_OUTPUT"
fi
