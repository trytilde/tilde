-- This calendar month's priced spend on a bank's synthesis calls.
--! run (bank) : (cost_micros)
SELECT COALESCE(SUM(cost_micros),0)::BIGINT AS cost_micros FROM inference_requests
WHERE agent_id IS NULL AND memory_bank_id = :bank AND created_at >= date_trunc('month', NOW());
