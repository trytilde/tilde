--: Record(personal_connection_id?)

--! run : Record
-- A detached call abandoned by a crashed process is aborted after an hour; a wait for a reply is
-- never abandoned (its output is the next message, however late).
SELECT t.* FROM chat_tool_calls t JOIN chat_invocations i ON i.id=t.invocation_id WHERE t.status='running' AND (i.status<>'running' OR i.lease_expires_at<=NOW())
  AND (NOT t.detached OR (t.updated_at < NOW() - INTERVAL '1 hour' AND t.name <> 'thread.wait_for_reply')) ORDER BY t.thread_id,t.id LIMIT 100;
