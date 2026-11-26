--: Record(personal_connection_id?, heartbeat_at?)

--! run : Record
-- Calls whose invocation ended under them. A detached call goes on past its invocation: a
-- pending one while its process heartbeats it (a minute's grace), a background one for an hour
-- (whatever crashed under it), and a wait for a reply while a checkpoint awaits it, however late.
SELECT t.* FROM chat_tool_calls t JOIN chat_invocations i ON i.id=t.invocation_id WHERE t.status='running' AND (i.status<>'running' OR i.lease_expires_at<=NOW())
  AND (NOT t.detached
       OR (t.heartbeat_at IS NOT NULL AND t.heartbeat_at < NOW() - INTERVAL '1 minute')
       OR (t.heartbeat_at IS NULL AND t.name <> 'thread.wait_for_reply' AND t.updated_at < NOW() - INTERVAL '1 hour')
       OR (t.name = 'thread.wait_for_reply'
           AND NOT EXISTS (SELECT 1 FROM chat_run_checkpoints c WHERE t.id = ANY(c.pending_call_ids))))
ORDER BY t.thread_id,t.id LIMIT 100;
