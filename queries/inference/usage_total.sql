--: Record()

--! run (p1) : Record
SELECT COALESCE(SUM(cost_micros),0)::BIGINT AS cost_micros FROM inference_requests WHERE agent_id=:p1;
