--! run (p1, p2, p3, p4)
-- The caller holds the thread lock. One checkpoint per run: a later suspension replaces it.
INSERT INTO chat_run_checkpoints(run_id, invocation_id, state_json, pending_call_ids)
VALUES (:p1, :p2, :p3, :p4)
ON CONFLICT (run_id) DO UPDATE SET invocation_id = EXCLUDED.invocation_id, state_json = EXCLUDED.state_json,
    pending_call_ids = EXCLUDED.pending_call_ids, created_at = NOW();
