--! run (p1, p2, p3)
UPDATE chat_participants SET active=:p3 WHERE thread_id=:p1 AND id=:p2 AND active<>:p3;
