--: Record()

--! run (p1?, p2?, p3) : Record
SELECT t.id,t.title,t.primary_agent_id FROM chat_threads t WHERE (:p1::UUID IS NULL OR EXISTS(SELECT 1 FROM chat_participants p WHERE p.thread_id=t.id AND p.agent_id=:p1)) AND (:p2::UUID IS NULL OR t.id>:p2) ORDER BY t.id LIMIT :p3;
