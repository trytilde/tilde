SELECT id,thread_id FROM chat_participants WHERE agent_id=$1 AND active ORDER BY thread_id,id;
