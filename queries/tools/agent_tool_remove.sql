--! run (p1, p2)
DELETE FROM agent_tools WHERE source_id=:p1 AND tool_name=:p2;
