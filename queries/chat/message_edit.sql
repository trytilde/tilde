--! run (p1, p2, p3)
UPDATE chat_messages SET text=:p2,status=:p3 WHERE id=:p1;
