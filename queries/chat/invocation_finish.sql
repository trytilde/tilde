--: Record()

--! run (p1, p2) : Record
UPDATE chat_invocations SET status=:p2,ended_at=NOW() WHERE id=:p1 AND status IN ('pending','running') RETURNING run_id,thread_id;
