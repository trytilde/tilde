SELECT id,thread_id FROM chat_invocations WHERE agent_id=$1 AND (status='running' OR ($2 AND status='pending')) ORDER BY thread_id,id;
