--: Record()

--! run (p1) : Record
-- The agent's tools that are discovered tools of a ready MCP-served connection.
SELECT s.slug,a.name,a.tool_name,a.is_async,a.summary,a.description,a.display,c.id AS connection_id,c.provider_id,c.type_id,
  t.description AS tool_description,t.input_schema_json,t.output_schema_json,
  t.read_only,t.destructive,t.idempotent,t.open_world
FROM agent_tool_sources s
JOIN agent_tools a ON a.source_id=s.id
JOIN connections c ON c.id=s.connection_id AND c.status='ready'
JOIN connection_types ct ON ct.provider_id=c.provider_id AND ct.type_id=c.type_id AND ct.mcp_credential IS NOT NULL
JOIN connection_tools t ON t.connection_id=c.id AND t.name=a.tool_name
WHERE s.agent_id=:p1
ORDER BY s.slug,a.name;
