#!/usr/bin/env bash
# Run a command with sccache when the developer opted in (RUSTC_WRAPPER=sccache, usually in
# .env.local), without making sccache or its S3 bucket a prerequisite. Ported from
# trytilde/api's scripts/run-cargo-dev.sh.
set -euo pipefail

if [[ "${RUSTC_WRAPPER:-}" != "sccache" ]]; then
	exec "$@"
fi

# sccache's AWS credential chain does not read every AWS CLI SSO profile shape. Resolve the
# active profile once and share the temporary session with sccache and every rustc child.
projected=false
if [[ -n "${SCCACHE_BUCKET:-}" && -n "${AWS_PROFILE:-}" && -z "${AWS_ACCESS_KEY_ID:-}" ]] &&
	command -v aws > /dev/null; then
	if credentials="$(aws configure export-credentials --profile "$AWS_PROFILE" --format env-no-export 2> /dev/null)"; then
		eval "$(sed 's/^/export /' <<< "$credentials")"
		projected=true
	fi
fi
export AWS_EC2_METADATA_DISABLED="${AWS_EC2_METADATA_DISABLED:-true}"

# A running server keeps the credentials it started with.
if [[ "$projected" == true ]] && command -v sccache > /dev/null; then
	sccache --stop-server > /dev/null 2>&1 || true
fi

if ! command -v sccache > /dev/null || ! sccache "$(rustup which rustc)" -vV > /dev/null 2>&1; then
	echo "WARN: sccache is unavailable; building without it." >&2
	export RUSTC_WRAPPER=
fi

exec "$@"
