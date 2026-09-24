--! run (p1, p2)
UPDATE chat_invocations SET execution_traceparent=:p2 WHERE id=:p1;
