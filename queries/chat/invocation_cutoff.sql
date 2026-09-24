--! run (p1, p2?)
UPDATE chat_invocations SET history_through_message_id=:p2 WHERE id=:p1 AND status='pending';
