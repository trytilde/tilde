--: Record()

--! run (p1?, p2?) : Record
-- The invocation catalog: an agent's (p1) or sandbox blueprint's (p2) tools on its ready
-- connections. MCP-served types and instances of a tool host's provider are listed by their own
-- queries.
SELECT s.slug,t.name,t.tool_name,t.is_async,t.summary,t.description,t.display,c.id AS connection_id,c.provider_id,c.type_id
FROM agent_tool_sources s
JOIN agent_tools t ON t.source_id=s.id
JOIN connections c ON c.id=s.connection_id AND c.status='ready'
JOIN connection_providers p ON p.provider_id=c.provider_id AND p.tool_host_id IS NULL
JOIN connection_types ct ON ct.provider_id=c.provider_id AND ct.type_id=c.type_id AND ct.mcp_credential IS NULL
WHERE (s.agent_id=:p1 OR s.sandbox_blueprint_id=:p2)
ORDER BY s.slug,t.name;
