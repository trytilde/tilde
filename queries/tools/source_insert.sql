--! run (p1, p2?, p3?, p4?, p5, p6?, p7?)
-- For an agent (p2) or a sandbox blueprint (p6); its target is a connection (p3), a tool host
-- (p4) or the agent's sandbox (p7). Inserts nothing when the owner already uses this slug or
-- this source.
INSERT INTO agent_tool_sources(id,agent_id,connection_id,tool_host_id,slug,sandbox_blueprint_id,sandbox_agent_id)
VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7) ON CONFLICT DO NOTHING;
