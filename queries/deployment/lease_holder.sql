--: Record(public_url?, deployment_id?)

--! run (p1, p2, p3) : Record
SELECT n.deployment_id AS deployment_id,l.instance_id,l.updated_at,n.public_url AS public_url,COALESCE(n.ready AND n.last_seen_at>NOW()-make_interval(secs=>:p3::float8),FALSE) AS live
FROM thread_leases l LEFT JOIN agent_instances n ON n.agent_id=l.agent_id AND n.instance_id=l.instance_id
WHERE l.thread_id=:p1 AND l.agent_id=:p2;
