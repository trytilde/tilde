-- This calendar month's priced spend on memory calls no agent or bank owns: background
-- embedding, its settings probe and recall from the console.
--! run : (cost_micros)
SELECT COALESCE(SUM(cost_micros),0)::BIGINT AS cost_micros FROM inference_requests
WHERE agent_id IS NULL AND memory_bank_id IS NULL AND created_at >= date_trunc('month', NOW());
