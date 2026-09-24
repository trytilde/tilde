--: Record()

--! run (p1, p2) : Record
WITH policy AS (
 SELECT s.routing,s.serving_deployment_id FROM agent_deployment_settings s
 JOIN agents a ON a.id=s.agent_id WHERE s.agent_id=:p1 AND a.deleted_at IS NULL
), available AS (
 SELECT d.id,d.created_at,d.traffic_weight FROM agent_deployments d
 WHERE d.agent_id=:p1 AND d.status='registered' AND (d.target='lambda' OR EXISTS (
 SELECT 1 FROM agent_instances n WHERE n.agent_id=d.agent_id AND n.deployment_id=d.id
 AND n.connection_id IS NOT NULL AND n.ready AND n.agent_ready AND n.agent_connected
 AND n.last_seen_at>NOW()-make_interval(secs=>:p2::float8)
))
), candidates AS (
 SELECT id,SUM(traffic_weight) OVER (ORDER BY id) AS ceiling FROM available WHERE traffic_weight>0
), ticket AS MATERIALIZED (SELECT random() * COALESCE(MAX(ceiling),0) AS value FROM candidates)
SELECT d.id FROM available d CROSS JOIN policy p WHERE d.id = CASE WHEN p.routing='weighted'
 THEN (SELECT c.id FROM candidates c CROSS JOIN ticket t WHERE c.ceiling>t.value ORDER BY c.ceiling LIMIT 1)
 ELSE COALESCE((SELECT id FROM available WHERE id=p.serving_deployment_id),
               (SELECT id FROM available ORDER BY created_at DESC,id LIMIT 1)) END;
