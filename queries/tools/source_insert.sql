--! run (p1, p2, p3?, p4?, p5)
-- Inserts nothing when the agent already uses this slug or this source.
INSERT INTO agent_tool_sources(id,agent_id,connection_id,tool_host_id,slug) VALUES(:p1,:p2,:p3,:p4,:p5) ON CONFLICT DO NOTHING;
