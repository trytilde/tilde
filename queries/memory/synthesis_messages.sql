--: Record(user_id?, identity?)

--! run (p1, p2, p3) : Record
-- Up to p3 unsynthesized messages of thread p2 for bank p1, oldest first and before any still
-- streaming, each cut to 8000 characters (`truncated`); a person's carries their root identity.
SELECT m.id,m.created_at,left(m.text,8000) AS text,length(m.text)>8000 AS truncated,
  COALESCE(u.name,a.name,'') AS author,p.user_id,COALESCE(u.root_identity_id,u.id) AS identity
FROM chat_messages m JOIN chat_participants p ON p.id=m.participant_id
LEFT JOIN chat_users u ON u.id=p.user_id LEFT JOIN agents a ON a.id=p.agent_id
LEFT JOIN memory_synthesis_threads w ON w.bank_id=:p1 AND w.thread_id=:p2
WHERE m.thread_id=:p2 AND m.status='complete' AND m.text<>'' AND m.created_at<NOW()-interval '5 seconds'
  AND (w.thread_id IS NULL OR (m.created_at,m.id)>(w.through_at,w.through_id))
  AND NOT EXISTS (SELECT 1 FROM chat_messages s WHERE s.thread_id=m.thread_id AND s.status='streaming'
    AND (s.created_at,s.id)<=(m.created_at,m.id))
ORDER BY m.created_at,m.id LIMIT :p3;
