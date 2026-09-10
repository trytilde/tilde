SELECT participant_id,invocation_id FROM chat_messages WHERE id=$1 AND thread_id=$2;
