--! run (p1, p2)
UPDATE chat_messages SET format=:p2 WHERE id=:p1;
