--: Record()

--! run (p1) : Record
SELECT agent_id,run_id,thread_id,execution_traceparent FROM chat_invocations WHERE id=:p1;
