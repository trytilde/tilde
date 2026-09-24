--! run (p1, p2, p3)
UPDATE agent_deployments SET traffic_weight=:p3 WHERE agent_id=:p1 AND id=:p2;
