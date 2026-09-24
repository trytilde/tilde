--: Record()

--! run (p1, p2?, p3?) : Record
SELECT id,active FROM chat_participants WHERE thread_id=:p1 AND (user_id=:p2 OR agent_id=:p3) FOR UPDATE;
