--: Record(cost_micros?)

--! run (p1, p2, p3) : Record
SELECT connection_id,COUNT(*)::BIGINT AS requests,COALESCE(SUM(input_tokens),0)::BIGINT AS input_tokens,COALESCE(SUM(output_tokens),0)::BIGINT AS output_tokens,SUM(cost_micros)::BIGINT AS cost_micros,COUNT(*) FILTER (WHERE cost_micros IS NULL)::BIGINT AS unpriced_requests
FROM inference_requests WHERE agent_id=:p1 AND created_at>=:p2 AND created_at<:p3 GROUP BY connection_id ORDER BY connection_id;
