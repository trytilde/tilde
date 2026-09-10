UPDATE chat_messages SET text=text || $2,stream_expires_at=NOW()+INTERVAL '30 seconds' WHERE id=$1 AND status='streaming';
