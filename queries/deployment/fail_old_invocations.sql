--: Record()

--! run (p1, p2) : Record
UPDATE chat_invocations SET status='failed',ended_at=NOW() WHERE thread_id=:p1 AND agent_id=:p2 AND status IN ('pending','running') RETURNING id;
