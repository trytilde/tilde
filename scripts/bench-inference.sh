#!/usr/bin/env bash
# Compare the inference gateway's proxy overhead with LiteLLM and Bifrost against one shared
# mock upstream. Usage: scripts/bench-inference.sh [--requests N] [--concurrency C] [--stream true|false]
# Set OPENAI_API_KEY to add a small real-provider run (`--real-requests N`, default 20).
set -euo pipefail
cd "$(dirname "$0")/.."
REQUESTS=1000; CONCURRENCY=32; STREAM=true; REAL_REQUESTS=20; CHUNKS=20; DELAY_MS=0
while [ $# -gt 0 ]; do
  case "$1" in
    --requests) REQUESTS="$2"; shift 2;;
    --concurrency) CONCURRENCY="$2"; shift 2;;
    --stream) STREAM="$2"; shift 2;;
    --real-requests) REAL_REQUESTS="$2"; shift 2;;
    --chunks) CHUNKS="$2"; shift 2;;
    --delay-ms) DELAY_MS="$2"; shift 2;;
    *) echo "unknown flag $1" >&2; exit 2;;
  esac
done
export TILDE_BENCH_OUT=/tmp/tilde-inference-bench.json TILDE_BENCH_STOP=/tmp/tilde-inference-bench.stop
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target/prepare}"
rm -f "$TILDE_BENCH_OUT" "$TILDE_BENCH_STOP"
cleanup() {
  touch "$TILDE_BENCH_STOP"
  [ -n "${MOCK_PID:-}" ] && kill "$MOCK_PID" 2>/dev/null || true
  [ -n "${GATEWAY_PID:-}" ] && wait "$GATEWAY_PID" 2>/dev/null || true
  docker compose -f dev/inference-bench/compose.yaml down --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
echo "building release fixture"
cargo test --release -p tilde --test inference_bench --no-run 2>&1 | tail -1
node dev/inference-bench/mock-upstream.mjs --port 18300 --chunks "$CHUNKS" --delay-ms "$DELAY_MS" & MOCK_PID=$!
docker compose -f dev/inference-bench/compose.yaml up -d --quiet-pull 2>&1 | tail -2
scripts/with-postgres.sh cargo test --release -p tilde --test inference_bench -- --ignored --nocapture serve_bench_gateway > /tmp/tilde-inference-bench.log 2>&1 & GATEWAY_PID=$!
for _ in $(seq 1 900); do [ -f "$TILDE_BENCH_OUT" ] && break; sleep 1; done
[ -f "$TILDE_BENCH_OUT" ] || { echo "gateway fixture did not start"; tail -30 /tmp/tilde-inference-bench.log; exit 1; }
TILDE_URL="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["url"])' "$TILDE_BENCH_OUT")"
TILDE_TOKEN="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["token"])' "$TILDE_BENCH_OUT")"
wait_http() { for _ in $(seq 1 90); do curl -fsS -o /dev/null "$1" 2>/dev/null && return 0; sleep 1; done; echo "not ready: $1" >&2; return 1; }
wait_http http://127.0.0.1:14000/health/liveliness
wait_http http://127.0.0.1:18080/metrics || wait_http http://127.0.0.1:18080/ || true
echo; echo "== mock upstream, streaming=$STREAM, $CHUNKS chunks, ${DELAY_MS}ms between chunks =="
node dev/inference-bench/bench.mjs --requests "$REQUESTS" --concurrency "$CONCURRENCY" --stream "$STREAM" --json /tmp/tilde-inference-bench-results.json \
  --target "direct|http://127.0.0.1:18300/v1|gpt-mock|Bearer sk-mock" \
  --target "tilde|$TILDE_URL/inference/openai/mock|gpt-mock|Bearer $TILDE_TOKEN" \
  --target "litellm|http://127.0.0.1:14000/v1|gpt-mock|Bearer sk-bench-master" \
  --target "bifrost|http://127.0.0.1:18080/v1|openai/gpt-mock|Bearer sk-mock"
if [ -n "${OPENAI_API_KEY:-}" ] && [ "$REAL_REQUESTS" -gt 0 ]; then
  echo; echo "== real OpenAI (gpt-5-nano), $REAL_REQUESTS requests =="
  node dev/inference-bench/bench.mjs --requests "$REAL_REQUESTS" --concurrency 4 --warmup 2 --stream "$STREAM" \
    --target "direct|https://api.openai.com/v1|gpt-5-nano|Bearer $OPENAI_API_KEY" \
    --target "tilde|$TILDE_URL/inference/openai/real|gpt-5-nano|Bearer $TILDE_TOKEN" \
    --target "litellm|http://127.0.0.1:14000/v1|gpt-real|Bearer sk-bench-master" \
    --target "bifrost|http://127.0.0.1:18080/v1|openai/gpt-5-nano|Bearer sk-mock"
fi
