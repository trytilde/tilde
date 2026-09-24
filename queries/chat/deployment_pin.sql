--: Record(deployment_id?)

--! run (p1, p2, p3?) : Record
WITH current AS (
 SELECT p.deployment_id FROM chat_participants p JOIN agent_deployments d ON d.id=p.deployment_id AND d.status='registered'
 WHERE p.thread_id=:p1 AND p.agent_id=:p2 AND p.active
), chosen AS (
 SELECT COALESCE((SELECT deployment_id FROM current),:p3::uuid) AS id
)
UPDATE chat_participants p SET deployment_id=chosen.id FROM chosen WHERE p.thread_id=:p1 AND p.agent_id=:p2 AND p.active RETURNING p.deployment_id AS deployment_id;
