WITH e AS (DELETE FROM sidecar_events WHERE archived_at<NOW()-INTERVAL '7 days'),
 d AS (DELETE FROM sidecar_directives WHERE acked_at<NOW()-INTERVAL '1 hour' OR created_at<NOW()-INTERVAL '7 days'),
 n AS (DELETE FROM agent_instances WHERE last_seen_at<NOW()-INTERVAL '1 day' AND NOT EXISTS(SELECT 1 FROM thread_leases l WHERE l.agent_id=agent_instances.agent_id AND l.instance_id=agent_instances.instance_id))
SELECT 1 AS "ok!";
