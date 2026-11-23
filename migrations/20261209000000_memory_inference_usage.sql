-- Calls Tilde makes itself are recorded too (`inference::Payer`): a recall an agent's memory tool
-- runs keeps its invocation, so it counts toward that agent's usage and budgets; synthesis has
-- no invocation and names its bank; background embedding has neither and is the
-- organization's own spend.
ALTER TABLE inference_requests
    ALTER COLUMN agent_id DROP NOT NULL,
    ALTER COLUMN invocation_id DROP NOT NULL,
    ALTER COLUMN thread_id DROP NOT NULL,
    ALTER COLUMN run_id DROP NOT NULL,
    ALTER COLUMN participant_id DROP NOT NULL,
    ADD COLUMN memory_bank_id UUID;
CREATE INDEX inference_requests_memory ON inference_requests(memory_bank_id, created_at DESC) WHERE agent_id IS NULL;
