-- The stable idempotency key uses the originating invocation, while scheduling
-- follows the current invocation as events move between queued turns.
CREATE INDEX chat_inputs_by_invocation ON chat_inputs(invocation_id,sequence);
