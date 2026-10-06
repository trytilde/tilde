--: Record()

--! run (p1, p2?) : Record
-- The invocation a call from inside sandbox p1 is traced in: that of the operation it runs under
-- (p2, while the operation is in flight), else the sandbox's latest. The operation's trace context
-- parents the call; empty otherwise.
SELECT i.id AS invocation_id,i.agent_id,i.run_id,i.thread_id,
  COALESCE(c.traceparent,'') AS traceparent,COALESCE(c.tracestate,'') AS tracestate
FROM sandboxes s
LEFT JOIN sandbox_calls c ON c.id=:p2 AND c.sandbox_id=s.id
JOIN chat_invocations i ON i.id=COALESCE(c.invocation_id,s.invocation_id)
WHERE s.id=:p1;
