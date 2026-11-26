--: Record()

--! run : Record
-- Suspended runs whose pending calls have all finished, in threads where the agent runs nothing
-- else now (a message that arrived meanwhile started its own run; this one follows it).
SELECT c.run_id FROM chat_run_checkpoints c
JOIN chat_runs r ON r.id = c.run_id AND r.status = 'waiting'
JOIN agents a ON a.id = r.agent_id AND NOT a.paused AND a.deleted_at IS NULL
WHERE NOT EXISTS (SELECT 1 FROM chat_tool_calls t WHERE t.id = ANY(c.pending_call_ids) AND t.status = 'running')
  AND NOT EXISTS (SELECT 1 FROM chat_invocations i WHERE i.thread_id = r.thread_id AND i.agent_id = r.agent_id
                  AND i.status IN ('pending', 'running'))
ORDER BY c.created_at LIMIT 50;
