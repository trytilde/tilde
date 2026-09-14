SELECT p.deployment_id AS "deployment_id?" FROM chat_participants p JOIN agent_deployments d ON d.id=p.deployment_id AND d.status='registered' WHERE p.thread_id=$1 AND p.agent_id=$2 AND p.active;
