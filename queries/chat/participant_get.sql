--: Record(user_id?, agent_id?)

--! run (p1, p2) : Record
SELECT p.id,p.active,p.user_id,p.agent_id,COALESCE(u.name,a.name) AS name FROM chat_participants p LEFT JOIN chat_users u ON u.id=p.user_id LEFT JOIN agents a ON a.id=p.agent_id WHERE p.thread_id=:p1 AND p.id=:p2;

