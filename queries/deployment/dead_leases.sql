SELECT l.thread_id,l.agent_id,l.instance_id,d.failure_mode,n.deployment_id AS "deployment_id?",
 EXISTS(SELECT 1 FROM chat_invocations i WHERE i.thread_id=l.thread_id AND i.agent_id=l.agent_id AND i.status IN ('pending','running')) AS "active!"
FROM thread_leases l JOIN agents a ON a.id=l.agent_id JOIN agent_deployment_settings d ON d.agent_id=l.agent_id
LEFT JOIN agent_instances n ON n.agent_id=l.agent_id AND n.instance_id=l.instance_id
WHERE a.deployment_mode='sidecar' AND a.deleted_at IS NULL
AND (n.instance_id IS NULL OR NOT n.ready OR n.last_seen_at<NOW()-make_interval(secs=>$1::float8))
ORDER BY l.updated_at LIMIT 200;
