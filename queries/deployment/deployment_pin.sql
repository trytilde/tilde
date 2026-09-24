--: Record(deployment_id?)

--! run (p1, p2) : Record
SELECT p.deployment_id AS deployment_id FROM chat_participants p JOIN agent_deployments d ON d.id=p.deployment_id AND d.status='registered' WHERE p.thread_id=:p1 AND p.agent_id=:p2 AND p.active;
