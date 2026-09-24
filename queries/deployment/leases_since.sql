--: Record(public_url?)

--! run (p1, p2) : Record
SELECT l.thread_id,l.instance_id,l.updated_at,n.public_url AS public_url
FROM thread_leases l LEFT JOIN agent_instances n ON n.agent_id=l.agent_id AND n.instance_id=l.instance_id
WHERE l.agent_id=:p1 AND l.updated_at>:p2 ORDER BY l.updated_at,l.thread_id;
