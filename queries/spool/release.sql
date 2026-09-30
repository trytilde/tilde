-- A failed delivery gives the batch back after a short delay instead of holding its lease.
--! run (object_key, delay_seconds)
UPDATE telemetry_objects SET leased_until=NOW()+make_interval(secs=>:delay_seconds::INTEGER) WHERE object_key=:object_key;
