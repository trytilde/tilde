--: Record()

--! run (p1, p2) : Record
SELECT id,objective,status FROM chat_goals WHERE thread_id=:p1 AND agent_id=:p2 ORDER BY id;
