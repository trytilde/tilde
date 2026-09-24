--: Record()

--! run (p1) : Record
SELECT p.agent_id AS agent_id FROM chat_participants p JOIN agents a ON a.id=p.agent_id WHERE p.thread_id=:p1 AND p.active AND a.deleted_at IS NULL ORDER BY p.id;
