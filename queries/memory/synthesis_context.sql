--: Record(user_id?, identity?)

--! run (p1, p2, p3) : Record
-- The p3 messages of thread p2 at or before bank p1's watermark there, oldest first, each cut to
-- 1000 characters: context for a batch, not evidence. A person's carries their root identity.
SELECT * FROM (
  SELECT m.created_at,left(m.text,1000) AS text,COALESCE(u.name,a.name,'') AS author,p.user_id,
    COALESCE(u.root_identity_id,u.id) AS identity
  FROM chat_messages m JOIN chat_participants p ON p.id=m.participant_id
  LEFT JOIN chat_users u ON u.id=p.user_id LEFT JOIN agents a ON a.id=p.agent_id
  JOIN memory_synthesis_threads w ON w.bank_id=:p1 AND w.thread_id=:p2
  WHERE m.thread_id=:p2 AND m.status='complete' AND m.text<>'' AND (m.created_at,m.id)<=(w.through_at,w.through_id)
  ORDER BY m.created_at DESC,m.id DESC LIMIT :p3) c
ORDER BY created_at;
