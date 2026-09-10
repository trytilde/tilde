SELECT sequence,kind,entity_id,text_delta,snapshot,created_at FROM chat_activity WHERE thread_id=$1 AND sequence>$2 ORDER BY sequence LIMIT $3;
