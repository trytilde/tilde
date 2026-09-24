--: Record()

--! run (p1, p2) : Record
SELECT id FROM chat_participants WHERE thread_id=:p1 AND agent_id=:p2 AND active LIMIT 1;
