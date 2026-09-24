--: Record()

--! run (p1, p2) : Record
SELECT id,thread_id FROM chat_invocations WHERE agent_id=:p1 AND (status='running' OR (:p2 AND status='pending')) ORDER BY thread_id,id;
