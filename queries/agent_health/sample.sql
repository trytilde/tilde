--! run (p1)
-- One aggregate observation per agent, including agents with no deployment. Instance
-- heartbeats describe health; an open Watch and local agent connection prove liveness.
WITH availability AS (
 SELECT d.id,d.agent_id,d.created_at,d.traffic_weight,
 d.target='lambda' OR EXISTS (
  SELECT 1 FROM agent_instances n WHERE n.agent_id=d.agent_id AND n.deployment_id=d.id
  AND n.connection_id IS NOT NULL AND n.ready AND n.agent_ready AND n.agent_connected
  AND n.last_seen_at>:p1::timestamptz-INTERVAL '15 seconds'
 ) AS ready
 FROM agent_deployments d WHERE d.status='registered'
), samples AS (
 SELECT a.id,COALESCE(CASE WHEN s.routing='weighted' THEN (
  SELECT BOOL_OR(d.ready) FROM availability d WHERE d.agent_id=a.id AND d.traffic_weight>0
 ) ELSE (
  SELECT d.ready FROM availability d WHERE d.agent_id=a.id AND d.ready
  ORDER BY (d.id=s.serving_deployment_id) DESC,d.created_at DESC,d.id LIMIT 1
 ) END,FALSE) AS healthy,
 COALESCE(s.routing='weighted' AND EXISTS (SELECT 1 FROM availability d WHERE d.agent_id=a.id AND d.traffic_weight>0 AND d.ready)
 AND EXISTS (SELECT 1 FROM availability d WHERE d.agent_id=a.id AND d.traffic_weight>0 AND NOT d.ready),FALSE) AS degraded
 FROM agents a LEFT JOIN agent_deployment_settings s ON s.agent_id=a.id WHERE a.deleted_at IS NULL
)
INSERT INTO agent_health(agent_id,checked_at,healthy,degraded,latency_ms,error_code)
SELECT id,:p1,healthy,degraded,0,CASE WHEN healthy THEN NULL ELSE 'not_ready' END FROM samples;
