--: Record()

--! run (p1, p2, p3) : Record
SELECT EXISTS(SELECT 1 FROM agent_instances WHERE agent_id=:p1 AND instance_id=:p2 AND ready AND last_seen_at>NOW()-make_interval(secs=>:p3::float8)) AS live;
