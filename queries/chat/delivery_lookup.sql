--: Record()

--! run (p1, p2, p3) : Record
SELECT d.message_id FROM chat_message_deliveries d JOIN chat_messages m ON m.id=d.message_id WHERE d.connection_id=:p1 AND d.external_message_id=:p2 AND m.thread_id=:p3;
