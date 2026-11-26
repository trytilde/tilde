--: Record()

--! run : Record
-- Invocations that can suspend, at their deadline: the run moves to `suspending`, which the invocation's control
-- stream delivers as a suspend command.
WITH due AS (
    UPDATE chat_runs r SET status = 'suspending' FROM chat_invocations i
    WHERE i.run_id = r.id AND i.status = 'running' AND i.suspendable AND i.suspend_at <= NOW() AND r.status = 'active'
    RETURNING i.id, i.thread_id
)
UPDATE chat_invocations i SET suspended_for = COALESCE(i.suspended_for, 'deadline') FROM due
WHERE i.id = due.id RETURNING i.id, i.thread_id;
