SELECT snapshot FROM chat_activity WHERE thread_id=$1 AND entity_id=$2 AND kind='message.completed' ORDER BY sequence DESC LIMIT 1;
