SELECT id,text FROM chat_inputs WHERE invocation_id=$1 AND NOT accepted ORDER BY id;
