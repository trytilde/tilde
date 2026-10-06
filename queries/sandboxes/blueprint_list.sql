--: Record()

--! run (p1?, p2?) : Record
-- One blueprint (p1) or all, optionally searched by name (p2).
SELECT b.id,b.name,b.connection_id,b.template,b.reuse,b.sleep_after_secs,b.terminate_after_secs,b.connect_timeout_secs,
  COALESCE((SELECT array_agg(e.name ORDER BY e.name) FROM sandbox_blueprint_env e WHERE e.blueprint_id=b.id), '{}')::TEXT[] AS env_names,
  COALESCE((SELECT array_agg(a.agent_id ORDER BY a.agent_id) FROM agent_sandboxes a WHERE a.blueprint_id=b.id), '{}')::UUID[] AS agent_ids
FROM sandbox_blueprints b
WHERE (:p1::UUID IS NULL OR b.id=:p1) AND (:p2::TEXT IS NULL OR b.name ILIKE '%'||:p2||'%')
ORDER BY b.name;
