--: Record()

--! run (p1) : Record
SELECT b.id,b.connection_id,c.provider_id||'/'||c.name AS connection_slug,b.template,b.reuse,b.sleep_after_secs,
  b.terminate_after_secs,b.connect_timeout_secs
FROM sandbox_blueprints b JOIN connections c ON c.id=b.connection_id WHERE b.name=:p1;
