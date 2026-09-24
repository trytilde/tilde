--! run (p1, p2)
DELETE FROM chat_typing WHERE thread_id=:p1 AND participant_id=:p2;
