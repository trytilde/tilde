--: Record()

--! run (p1, p2) : Record
UPDATE agent_deployment_settings s SET serving_deployment_id=d.id FROM agent_deployments d
WHERE s.agent_id=:p1 AND d.id=:p2 AND d.agent_id=:p1 AND d.status='registered' RETURNING d.id;
