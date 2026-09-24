--: Record()

--! run (p1, p2) : Record
SELECT id,run_id,status FROM chat_invocations WHERE thread_id=:p1 AND agent_id=:p2 AND status IN ('pending','running');
