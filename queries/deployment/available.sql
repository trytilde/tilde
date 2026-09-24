--: Record()

--! run (p1, p2) : Record
SELECT d.id FROM agent_deployments d WHERE d.agent_id=:p1 AND d.status='registered' AND (d.target='lambda' OR EXISTS (
 SELECT 1 FROM agent_instances n WHERE n.agent_id=d.agent_id AND n.deployment_id=d.id
 AND n.connection_id IS NOT NULL AND n.ready AND n.agent_ready AND n.agent_connected
 AND n.last_seen_at>NOW()-make_interval(secs=>:p2::float8)
));
