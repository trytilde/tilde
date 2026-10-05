--: Record()

--! run (p1) : Record
-- The agent's tools from its sandbox.
SELECT s.slug,a.name,a.tool_name,a.is_async,a.summary,a.description,a.display
FROM agent_tool_sources s JOIN agent_tools a ON a.source_id=s.id
WHERE s.agent_id=:p1 AND s.sandbox_agent_id IS NOT NULL
ORDER BY a.name;
