--! run (p1, p2) : (thread_id)
-- The conversation with unsynthesized messages that waited longest, among those of agents that
-- auto-retain into bank p1. Its messages are read before any still streaming, and once the
-- conversation is idle (no invocation of the agent pending or running there, and its newest
-- message twenty minutes old), or once p2 characters wait in a conversation the gateway runs.
-- Conversations a sidecar runs reach the gateway projected, so they wait until idle.
SELECT p.thread_id
FROM chat_participants p
JOIN agent_memory_banks l ON l.agent_id=p.agent_id AND l.bank_id=:p1 AND l.auto_retain
LEFT JOIN memory_synthesis_threads w ON w.bank_id=:p1 AND w.thread_id=p.thread_id
CROSS JOIN LATERAL (
  SELECT COALESCE(SUM(length(m.text)),0) AS chars,MIN(m.created_at) AS oldest FROM chat_messages m
  WHERE m.thread_id=p.thread_id AND m.status='complete' AND m.text<>''
    AND m.created_at<NOW()-interval '5 seconds'
    AND (w.thread_id IS NULL OR (m.created_at,m.id)>(w.through_at,w.through_id))
    AND NOT EXISTS (SELECT 1 FROM chat_messages s WHERE s.thread_id=m.thread_id AND s.status='streaming'
      AND (s.created_at,s.id)<=(m.created_at,m.id))) pending
WHERE pending.chars>0 AND (
  (NOT EXISTS (SELECT 1 FROM chat_invocations i WHERE i.thread_id=p.thread_id AND i.agent_id=p.agent_id
     AND i.status IN ('pending','running'))
   AND (SELECT MAX(m.created_at) FROM chat_messages m WHERE m.thread_id=p.thread_id)<NOW()-interval '20 minutes')
  OR (pending.chars>=:p2 AND NOT EXISTS (SELECT 1 FROM thread_leases t WHERE t.thread_id=p.thread_id)))
ORDER BY pending.oldest LIMIT 1;
