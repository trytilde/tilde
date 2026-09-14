SELECT l.instance_id,n.public_url AS "public_url?",COALESCE(n.last_seen_at>NOW()-INTERVAL '15 seconds',FALSE) AS "live!"
FROM thread_leases l LEFT JOIN agent_instances n ON n.agent_id=l.agent_id AND n.instance_id=l.instance_id
WHERE l.thread_id=$1 AND l.agent_id=$2;
