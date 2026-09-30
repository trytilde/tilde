--: Record()
-- The lease hides the pointer from other replicas while this one delivers; a crashed or failed
-- delivery simply becomes due again when the lease runs out.
--! run (queue, lease_seconds) : Record
UPDATE telemetry_objects SET leased_until=NOW()+make_interval(secs=>:lease_seconds::INTEGER)
WHERE object_key=(SELECT object_key FROM telemetry_objects WHERE queue=:queue AND leased_until<=NOW()
 ORDER BY leased_until LIMIT 1 FOR UPDATE SKIP LOCKED)
RETURNING object_key;
