--: Record(exhausted_until?, connection_id?)

--! run (p1?, p2?) : Record
SELECT id,scope,scope_id,connection_id,period,limit_micros,action,spent_micros,exhausted_until,created_at,updated_at FROM inference_budgets
WHERE (:p1::TEXT IS NULL OR scope=:p1) AND (:p2::UUID IS NULL OR scope_id=:p2) ORDER BY scope,scope_id,connection_id,period;
