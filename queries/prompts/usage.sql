--: Record(cost_micros?)

--! run (p1) : Record
SELECT v.id AS version_id,COUNT(*)::BIGINT AS requests,COALESCE(SUM(r.input_tokens),0)::BIGINT AS input_tokens,COALESCE(SUM(r.output_tokens),0)::BIGINT AS output_tokens,
 SUM(r.cost_micros)::BIGINT AS cost_micros,COALESCE(AVG(r.latency_ms),0)::BIGINT AS average_latency_ms,MAX(r.created_at) AS last_used_at
FROM prompt_versions v JOIN inference_request_prompts l ON l.version_id=v.id JOIN inference_requests r ON r.id=l.request_id
WHERE v.prompt_id=:p1 GROUP BY v.id;
