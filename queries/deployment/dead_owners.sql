SELECT p.thread_id,p.participant_id,p.agent_id,p.owner_instance_id,p.generation,d.failure_mode
FROM participant_assignments p JOIN agents a ON a.id=p.agent_id JOIN agent_deployments d ON d.agent_id=p.agent_id
LEFT JOIN sidecar_nodes n ON n.agent_id=p.agent_id AND n.instance_id=p.owner_instance_id
WHERE NOT p.stopped AND a.deployment_mode='sidecar' AND a.deleted_at IS NULL AND NOT a.paused
AND (n.instance_id IS NULL OR NOT n.agent_ready OR n.last_seen_at<NOW()-INTERVAL '15 seconds')
AND EXISTS(SELECT 1 FROM chat_invocations i WHERE i.thread_id=p.thread_id AND i.agent_id=p.agent_id AND i.status IN ('pending','running'))
ORDER BY p.updated_at LIMIT 100;
