--! run (p1, p2)
UPDATE chat_messages SET text=text || :p2,stream_expires_at=NOW()+INTERVAL '30 seconds' WHERE id=:p1 AND status='streaming';
