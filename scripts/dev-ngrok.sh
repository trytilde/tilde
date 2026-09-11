#!/usr/bin/env bash
# Task owns the process lifetime and loads dotenv values as data, including the token.
set -euo pipefail

fail() { echo "ngrok: $*" >&2; exit 1; }
case "${NGROK_ENABLED:-false}" in
  false) exit 0 ;;
  true) ;;
  *) fail 'NGROK_ENABLED must be true or false' ;;
esac

[[ -n "${NGROK_AUTHTOKEN:-}" ]] || fail 'Set NGROK_AUTHTOKEN in .env.local or load it with task secrets:load'
# Require a bare DNS domain so the advertised origin and ngrok endpoint agree.
[[ "${NGROK_DOMAIN:-}" =~ ^[a-zA-Z0-9]([a-zA-Z0-9.-]*[a-zA-Z0-9])?$ ]] || fail 'Set NGROK_DOMAIN to your ngrok hostname (without https:// or a path)'
command -v ngrok >/dev/null || fail 'Install the ngrok CLI before enabling NGROK_ENABLED'

if [[ "${1:-}" == --check ]]; then exit 0; fi
host="${ADDRESS:-127.0.0.1}"
case "$host" in
  0.0.0.0) host=127.0.0.1 ;;
  ::|'[::]') host='[::1]' ;;
esac
# Forward only to event ingress. Management and browser setup remain private.
# Do not pool this endpoint: callbacks must reach this developer's database.
exec ngrok http "http://$host:${INGRESS_PORT:-8082}" --url "https://$NGROK_DOMAIN" --log stdout
