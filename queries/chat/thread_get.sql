--: Record()

--! run (p1) : Record
SELECT id,title,primary_agent_id FROM chat_threads WHERE id=:p1;
