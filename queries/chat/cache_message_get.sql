--: Record()

--! run (p1, p2) : Record
SELECT id,status FROM chat_messages WHERE id=:p1 AND thread_id=:p2;
