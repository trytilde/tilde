SELECT i.id,i.run_id,i.thread_id,i.agent_id,i.status,r.objective,COALESCE(d.endpoint_url,a.endpoint_url) AS "endpoint_url?",a.webhook_signing_key,
 i.deployment_id AS "deployment_id?",d.target AS "target?",d.target_reference AS "target_reference?"
FROM chat_invocations i JOIN chat_runs r ON r.id=i.run_id JOIN agents a ON a.id=i.agent_id LEFT JOIN agent_deployments d ON d.id=i.deployment_id WHERE i.id=$1;
