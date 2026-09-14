#!/usr/bin/env bash
set -euo pipefail

# Task normally gives exported variables precedence over task-level env values.
# The enabled tunnel is authoritative for webhook URLs, including exported overrides.
if [[ "${NGROK_ENABLED:-false}" == true ]]; then
  export ENGINE_INGRESS_PUBLIC_URL="https://${NGROK_DOMAIN:?Set NGROK_DOMAIN}"
fi

exec cargo run --bin tilde -- "$@"
