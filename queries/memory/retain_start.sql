--! run (p1, p2)
-- Agent p1 starts retaining into bank p2: its conversations count from their newest message on,
-- so existing history is not synthesized. A conversation another agent already retains into the
-- bank keeps its watermark.
INSERT INTO memory_synthesis_threads(bank_id,thread_id,through_at,through_id)
SELECT :p2,p.thread_id,n.created_at,n.id FROM chat_participants p
CROSS JOIN LATERAL (SELECT m.created_at,m.id FROM chat_messages m WHERE m.thread_id=p.thread_id
  ORDER BY m.created_at DESC,m.id DESC LIMIT 1) n
WHERE p.agent_id=:p1 AND NOT EXISTS (SELECT 1 FROM chat_participants o
  JOIN agent_memory_banks l ON l.agent_id=o.agent_id AND l.bank_id=:p2 AND l.auto_retain
  WHERE o.thread_id=p.thread_id AND o.agent_id<>:p1)
ON CONFLICT(bank_id,thread_id) DO UPDATE SET through_at=EXCLUDED.through_at,through_id=EXCLUDED.through_id;
