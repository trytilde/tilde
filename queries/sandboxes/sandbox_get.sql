--: Record(agent_id?, thread_id?, identity_id?, provider_sandbox_id?, snapshot_id?)

--! run (p1, p2, p3) : Record
-- `live`: its process's Connect stream refreshed connected_at within p2 seconds; `registered`:
-- its process connected after p3. `leased`: a process holds its lease. `template` is the
-- blueprint's current one, as are its timings.
SELECT s.id,s.blueprint_id,s.reuse,s.connection_id,s.agent_id,s.thread_id,s.identity_id,s.status,s.provider_sandbox_id,s.snapshot_id,s.error,
  COALESCE(s.connected_at > NOW() - make_interval(secs => :p2), false) AS live,
  COALESCE(s.connected_at > :p3, false) AS registered,
  COALESCE(s.lease_until > NOW(), false) AS leased,
  b.template,b.connect_timeout_secs
FROM sandboxes s JOIN sandbox_blueprints b ON b.id=s.blueprint_id WHERE s.id=:p1;
