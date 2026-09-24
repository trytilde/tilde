--! run (p1, p2?)
UPDATE agent_deployments SET traffic_weight=CASE WHEN id=:p2 THEN 100 ELSE 0 END WHERE agent_id=:p1;
