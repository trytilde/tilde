--: Record(cost_micros?)

--! run (p1, p2, p3, p4) : Record
SELECT date_trunc(:p4,created_at) AS bucket,connection_id,COUNT(*)::BIGINT AS requests,SUM(cost_micros)::BIGINT AS cost_micros
FROM inference_requests WHERE agent_id=:p1 AND created_at>=:p2 AND created_at<:p3 GROUP BY 1,2 ORDER BY 1,2;
