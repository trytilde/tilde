--: Record()

--! run (p1) : Record
SELECT id,thread_id FROM chat_participants WHERE agent_id=:p1 AND active ORDER BY thread_id,id;
