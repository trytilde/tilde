-- Suspended runs. A framework adapter that suspends its loop (a slow tool call answered
-- "pending", the invocation's deadline, a wait for a reply) saves the conversation in its model's
-- message format here and ends the invocation; the run waits, and once every call it waits for
-- has finished Tilde starts the run's next invocation, which loads the state and continues.
-- `state_json` is the SDK's, opaque to Tilde (like converted messages); `pending_call_ids` are
-- the tool calls (`chat_tool_calls`) whose results the next invocation is given.
CREATE TABLE chat_run_checkpoints (
    run_id UUID PRIMARY KEY REFERENCES chat_runs(id) ON DELETE CASCADE,
    invocation_id UUID NOT NULL REFERENCES chat_invocations(id) ON DELETE CASCADE,
    state_json TEXT NOT NULL,
    pending_call_ids UUID[] NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- When Tilde asks the invocation to suspend: before its host's runtime limit (Lambda's fifteen
-- minutes), with a margin to save. NULL where the host has no limit.
ALTER TABLE chat_invocations ADD COLUMN suspend_at TIMESTAMPTZ;
CREATE INDEX chat_invocations_suspend_at ON chat_invocations(suspend_at) WHERE status = 'running' AND suspend_at IS NOT NULL;
