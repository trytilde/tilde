--: Record()
--! run (queue, agent_id) : Record
SELECT COALESCE(SUM(bytes),0)::BIGINT AS bytes, COALESCE(SUM(objects),0)::BIGINT AS objects,
 COALESCE(SUM(bytes) FILTER (WHERE agent_id=:agent_id),0)::BIGINT AS agent_bytes
FROM telemetry_usage WHERE queue=:queue;
