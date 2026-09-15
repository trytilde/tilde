UPDATE chat_invocations SET history_through_message_id=$2 WHERE id=$1 AND status='pending';
