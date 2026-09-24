#!/usr/bin/env bash
# Domain callers use db modules; PostgreSQL SQL is checked by Cornucopia.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v rg >/dev/null || { echo "ripgrep is required for query-boundary checks" >&2; exit 1; }
if rg -n 'sqlx::|query_file!|query_file_as!' crates/tilde; then
  echo 'SQLx query calls are not allowed; use Cornucopia-backed db operations.' >&2
  exit 1
fi
if rg -n 'tilde_queries::queries' crates/tilde/src --glob '*.rs' --glob '!db.rs'; then
  echo 'Call generated PostgreSQL bindings through the owning domain db module.' >&2
  exit 1
fi
