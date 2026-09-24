--: Record()

--! run (p1, p2) : Record
SELECT EXISTS(SELECT 1 FROM chat_participants WHERE thread_id=:p1 AND agent_id=:p2 AND active) AS allowed;
