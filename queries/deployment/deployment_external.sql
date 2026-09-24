--: Record()

--! run (p1, p2) : Record
SELECT id FROM agent_deployments WHERE agent_id=:p1 AND external_id=:p2;
