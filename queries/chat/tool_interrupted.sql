SELECT t.* FROM chat_tool_calls t JOIN chat_invocations i ON i.id=t.invocation_id WHERE t.status='running' AND (i.status<>'running' OR i.lease_expires_at<=NOW()) ORDER BY t.thread_id,t.id LIMIT 100;
