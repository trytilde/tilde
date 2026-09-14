-- Pin the (thread, agent) pair to a deployment on first lease; a pin to a retired deployment is replaced.
WITH current AS (
 SELECT p.deployment_id FROM chat_participants p JOIN agent_deployments d ON d.id=p.deployment_id AND d.status='registered'
 WHERE p.thread_id=$1 AND p.agent_id=$2 AND p.active
), chosen AS (SELECT COALESCE((SELECT deployment_id FROM current),$3::uuid) AS id)
UPDATE chat_participants p SET deployment_id=chosen.id FROM chosen WHERE p.thread_id=$1 AND p.agent_id=$2 AND p.active RETURNING p.deployment_id AS "deployment_id?";
