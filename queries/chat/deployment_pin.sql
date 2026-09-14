WITH current AS (
 SELECT p.deployment_id FROM chat_participants p JOIN agent_deployments d ON d.id=p.deployment_id AND d.status='registered'
 WHERE p.thread_id=$1 AND p.agent_id=$2 AND p.active
), chosen AS (
 SELECT COALESCE((SELECT deployment_id FROM current),(SELECT serving_deployment_id FROM agent_deployment_settings WHERE agent_id=$2)) AS id
)
UPDATE chat_participants p SET deployment_id=chosen.id FROM chosen WHERE p.thread_id=$1 AND p.agent_id=$2 AND p.active RETURNING p.deployment_id AS "deployment_id?";
