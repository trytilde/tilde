-- This calendar month's priced spend on calls Tilde made itself for memory, outside any agent's
-- invocation: a bank's synthesis, or with no bank, the organization's background embedding.
--! run (bank?) : (cost_micros)
SELECT COALESCE(SUM(cost_micros),0)::BIGINT AS cost_micros FROM inference_requests
WHERE agent_id IS NULL AND memory_bank_id IS NOT DISTINCT FROM :bank
  AND created_at >= date_trunc('month', NOW());
