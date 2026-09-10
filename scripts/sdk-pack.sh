#!/usr/bin/env bash
set -euo pipefail
output="$(pwd)/dist/npm"
mkdir -p "$output"
for manifest in sdk/ts/packages/*/package.json; do
  if node --input-type=module -e 'import fs from "node:fs"; process.exit(JSON.parse(fs.readFileSync(process.argv[1])).private ? 0 : 1)' "$manifest"; then
    continue
  fi
  pnpm --dir "$(dirname "$manifest")" pack --pack-destination "$output"
done
