--! run (p1)
-- A deleted agent uses no connection: its tool sources, routines and personal allowances go
-- (its connection assignments go with `unassign`).
WITH sources AS (DELETE FROM agent_tool_sources WHERE agent_id=:p1),
routines AS (DELETE FROM routines WHERE agent_id=:p1)
DELETE FROM personal_connection_agents WHERE agent_id=:p1;
