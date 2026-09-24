#!/usr/bin/env bash
# Called inside disposable Postgres by task queries:generate. Ordinary builds use
# the committed generated crate and never need a database or the generator.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${DATABASE_URL:?Run task queries:generate to use disposable Postgres}"
if [[ "$(cornucopia --version)" != "cornucopia 1.0.1" ]]; then
  echo 'Install the pinned generator: cargo install cornucopia --version 1.0.1 --locked' >&2
  exit 1
fi
for migration in migrations/*.sql; do
  psql "$DATABASE_URL" -X -q -v ON_ERROR_STOP=1 -f "$migration" >/dev/null
done
if [[ "${1:-}" == --check ]]; then
  generated="$(mktemp -d .queries-check-XXXXXX)"
  trap 'rm -rf "$generated"' EXIT
  cornucopia live --destination "$generated/queries"
  diff -ru crates/queries "$generated/queries"
else
  cornucopia live
fi
