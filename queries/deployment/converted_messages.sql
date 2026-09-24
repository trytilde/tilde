--: Record()

--! run (p1, p2) : Record
SELECT c.message_id,c.representation FROM chat_converted_messages c JOIN chat_messages m ON m.id=c.message_id WHERE c.agent_id=:p1 AND m.thread_id=:p2 ORDER BY c.message_id;
