--: Record()

--! run (p1, p2, p3) : Record
SELECT instance_id FROM agent_instances WHERE agent_id=:p1 AND deployment_id=:p2 AND ready AND agent_ready AND last_seen_at>NOW()-make_interval(secs=>:p3::float8) ORDER BY last_seen_at DESC LIMIT 1;
