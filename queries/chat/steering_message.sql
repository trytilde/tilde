--: Record(snapshot?)

--! run (p1, p2) : Record
SELECT snapshot FROM chat_activity WHERE thread_id=:p1 AND entity_id=:p2 AND kind='message.completed' ORDER BY sequence DESC LIMIT 1;
