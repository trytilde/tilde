--: Record()

--! run (p1, p2) : Record
-- What an agent said during one run of this thread.
SELECT m.id,m.text FROM chat_messages m JOIN chat_invocations i ON i.id=m.invocation_id
WHERE i.run_id=:p1 AND m.thread_id=:p2 AND m.status='complete' ORDER BY m.created_at,m.id LIMIT 50;
