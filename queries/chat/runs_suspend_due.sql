--: Record()

--! run : Record
-- Invocations at their deadline: the run moves to `suspending`, which the invocation's control
-- stream delivers as a suspend command.
UPDATE chat_runs r SET status = 'suspending' FROM chat_invocations i
WHERE i.run_id = r.id AND i.status = 'running' AND i.suspend_at <= NOW() AND r.status = 'active'
RETURNING i.id, i.thread_id;
