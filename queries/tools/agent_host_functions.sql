--: Record(function_arn?, connection_id?, connection_type?)

--! run (p1?, p2, p3?) : Record
-- An agent's (p1) or sandbox blueprint's (p3) tools served by a tool host that can take a call
-- now: Lambda always, a connected host while its Watch stream refreshed connected_at within the
-- liveness window (p2 seconds).
-- A source is the host itself when it publishes no provider, or a ready installation-owned
-- instance (connection) of the provider it does publish.
SELECT s.slug,a.name,a.tool_name,a.is_async,a.summary,a.description,a.display,h.id AS tool_host_id,h.execution_type,h.function_arn,
  c.id AS connection_id,c.type_id AS connection_type,
  t.description AS tool_description,t.summary AS tool_summary,t.input_schema_json,t.output_schema_json,
  t.read_only,t.destructive,t.idempotent,t.open_world
FROM agent_tool_sources s
JOIN agent_tools a ON a.source_id=s.id
LEFT JOIN connections c ON c.id=s.connection_id
LEFT JOIN connection_providers p ON p.provider_id=c.provider_id
JOIN tool_hosts h ON h.id=COALESCE(s.tool_host_id,p.tool_host_id)
JOIN tool_host_tools t ON t.tool_host_id=h.id AND t.name=a.tool_name
WHERE (s.agent_id=:p1 OR s.sandbox_blueprint_id=:p3)
  AND (h.execution_type='lambda' OR h.connected_at > NOW() - make_interval(secs => :p2))
  AND CASE WHEN s.tool_host_id IS NOT NULL
    THEN NOT EXISTS (SELECT 1 FROM connection_providers hp WHERE hp.tool_host_id=h.id)
    ELSE c.status='ready' AND c.owner_user_id IS NULL END
ORDER BY s.slug,a.name;
