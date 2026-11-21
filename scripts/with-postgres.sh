#!/usr/bin/env bash
# Run a command against a disposable database on a random loopback port.
# WITH_PGBOUNCER=1 puts a transaction-pooling PgBouncer in front of it, configured as hosted
# poolers run one: TEST_DATABASE_URL (and DATABASE_URL) is then PgBouncer and TEST_DATABASE_DIRECT_URL
# (and DATABASE_DIRECT_URL) the server.
set -euo pipefail
container="tilde-test-$(date +%s)-$$"
bouncer="$container-pgbouncer"
network="$container"
config=""
cleanup(){
  docker rm --force --volumes "$bouncer" "$container" >/dev/null 2>&1 || true
  docker network rm "$network" >/dev/null 2>&1 || true
  if [[ -n "$config" ]]; then rm -rf "$config"; fi
}
trap cleanup EXIT INT TERM
# The pgvector image is Debian-based; the C locale keeps the byte-order text ordering the Alpine
# image gave, instead of en_US.
init=(--env 'POSTGRES_INITDB_ARGS=--locale=C --encoding=UTF8')
settings=(
  # A throwaway database needs no durability.
  -c fsync=off -c synchronous_commit=off -c full_page_writes=off
  # Bounded WAL keeps the whole suite, which creates and drops a schema per test, inside the
  # in-memory data directory.
  -c wal_level=minimal -c max_wal_senders=0 -c max_wal_size=256MB
  # Each test schema migrates in one transaction, which locks every object it creates; parallel
  # tests outgrow the default lock table.
  -c max_locks_per_transaction=256
)
network_args=()
image=pgvector/pgvector:pg16
data=/var/lib/postgresql/data
if [[ "${WITH_PGBOUNCER:-}" == 1 ]]; then
  docker network create "$network" >/dev/null
  network_args=(--network "$network")
  settings+=(-c max_connections=400)
  # PostgreSQL 18 reports search_path to clients, which is what lets PgBouncer
  # keep each test's schema with its client; on 16 a server session keeps an earlier client's.
  image=pgvector/pgvector:pg18
  data=/var/lib/postgresql
fi
docker run --detach --name "$container" "${network_args[@]}" --tmpfs "$data:rw,size=2g" --tmpfs /var/run/postgresql:rw,size=16m --env POSTGRES_USER=engine --env POSTGRES_PASSWORD=engine-test --env POSTGRES_DB=engine_test "${init[@]}" --publish 127.0.0.1::5432 "$image" \
  "${settings[@]}" >/dev/null
for attempt in {1..60}; do
  if docker exec "$container" pg_isready -h 127.0.0.1 -U engine -d engine_test >/dev/null 2>&1; then break; fi
  sleep 1
done
docker exec "$container" pg_isready -h 127.0.0.1 -U engine -d engine_test >/dev/null
port="$(docker port "$container" 5432/tcp | cut -d: -f2)"
export DATABASE_URL="postgres://engine:engine-test@127.0.0.1:${port}/engine_test"
export TEST_DATABASE_URL="$DATABASE_URL"
if [[ "${WITH_PGBOUNCER:-}" == 1 ]]; then
  # search_path is tracked so each test's `options=-c search_path=<schema>` follows its client
  # across server sessions; logins created at runtime (environment roles) authenticate through
  # auth_query.
  config="$(mktemp -d)"
  cat >"$config/pgbouncer.ini" <<INI
[databases]
* = host=$container port=5432 auth_user=engine
[pgbouncer]
listen_addr = 0.0.0.0
listen_port = 6432
auth_type = scram-sha-256
auth_file = /etc/pgbouncer/userlist.txt
auth_query = SELECT rolname, rolpassword FROM pg_authid WHERE rolname = \$1 AND rolcanlogin
pool_mode = transaction
max_prepared_statements = 500
max_client_conn = 5000
default_pool_size = ${PGBOUNCER_POOL_SIZE:-40}
track_extra_parameters = search_path
ignore_startup_parameters = extra_float_digits
INI
  echo '"engine" "engine-test"' >"$config/userlist.txt"
  chmod -R a+rX "$config"
  docker run --detach --name "$bouncer" --network "$network" --volume "$config:/etc/pgbouncer:ro" --publish 127.0.0.1::6432 \
    --entrypoint pgbouncer edoburu/pgbouncer:v1.26.1-p0 /etc/pgbouncer/pgbouncer.ini >/dev/null
  bouncer_port="$(docker port "$bouncer" 6432/tcp | cut -d: -f2)"
  for attempt in {1..30}; do
    if pg_isready -h 127.0.0.1 -p "$bouncer_port" >/dev/null 2>&1; then break; fi
    sleep 1
  done
  export DATABASE_DIRECT_URL="$DATABASE_URL" TEST_DATABASE_DIRECT_URL="$DATABASE_URL"
  export DATABASE_URL="postgres://engine:engine-test@127.0.0.1:${bouncer_port}/engine_test"
  export TEST_DATABASE_URL="$DATABASE_URL"
fi
"$@"
