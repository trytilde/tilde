--: Record()

--! run (p1) : Record
SELECT invocation_id, state_json, pending_call_ids FROM chat_run_checkpoints WHERE run_id = :p1;
