--: Record()

--! run (p1, p2?, p3?, p4) : Record
SELECT instance_id,public_url FROM agent_instances WHERE agent_id=:p1 AND instance_id<>COALESCE(:p2,'00000000-0000-0000-0000-000000000000'::uuid) AND (:p3::uuid IS NULL OR deployment_id=:p3) AND ready AND agent_ready AND last_seen_at>NOW()-make_interval(secs=>:p4::float8) ORDER BY instance_id LIMIT 1;
