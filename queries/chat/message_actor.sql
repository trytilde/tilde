--: Record(invocation_id?)

--! run (p1, p2) : Record
SELECT participant_id,invocation_id FROM chat_messages WHERE id=:p1 AND thread_id=:p2;
