--! run (p1, p2)
UPDATE chat_messages SET status=:p2 WHERE id=:p1 AND status='streaming';
