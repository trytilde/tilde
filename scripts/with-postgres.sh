#!/usr/bin/env bash
# Run a command against a disposable database on a random loopback port.
set -euo pipefail
container="tilde-test-$(date +%s)-$$"
cleanup(){ docker rm --force --volumes "$container" >/dev/null 2>&1 || true; }
trap cleanup EXIT INT TERM
# A throwaway database needs no durability. Bounded WAL keeps the whole suite, which creates and
# drops a schema per test, inside the in-memory data directory.
docker run --detach --name "$container" --tmpfs /var/lib/postgresql/data:rw,size=2g --tmpfs /var/run/postgresql:rw,size=16m --env POSTGRES_USER=engine --env POSTGRES_PASSWORD=engine-test --env POSTGRES_DB=engine_test --publish 127.0.0.1::5432 postgres:16-alpine \
  -c fsync=off -c synchronous_commit=off -c full_page_writes=off \
  -c wal_level=minimal -c max_wal_senders=0 -c max_wal_size=256MB >/dev/null
for attempt in {1..60}; do
  if docker exec "$container" pg_isready -h 127.0.0.1 -U engine -d engine_test >/dev/null 2>&1; then break; fi
  sleep 1
done
docker exec "$container" pg_isready -h 127.0.0.1 -U engine -d engine_test >/dev/null
port="$(docker port "$container" 5432/tcp | cut -d: -f2)"
export DATABASE_URL="postgres://engine:engine-test@127.0.0.1:${port}/engine_test"
export TEST_DATABASE_URL="$DATABASE_URL"
"$@"
