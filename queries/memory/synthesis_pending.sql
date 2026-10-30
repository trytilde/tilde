--: Record(agent_id?, wiki_id?)

--! run (p1) : Record
-- What waits to be synthesized into bank p1: conversations by agent, and pages by wiki.
SELECT p.agent_id,NULL::UUID AS wiki_id,COUNT(DISTINCT p.thread_id) AS count FROM chat_participants p
JOIN agent_memory_banks l ON l.agent_id=p.agent_id AND l.bank_id=:p1 AND l.auto_retain
LEFT JOIN memory_synthesis_threads w ON w.bank_id=:p1 AND w.thread_id=p.thread_id
WHERE EXISTS (SELECT 1 FROM chat_messages m WHERE m.thread_id=p.thread_id AND m.status='complete' AND m.text<>''
  AND (w.thread_id IS NULL OR (m.created_at,m.id)>(w.through_at,w.through_id)))
GROUP BY p.agent_id
UNION ALL
SELECT NULL::UUID,l.wiki_id,COUNT(*) FROM memory_bank_wikis l JOIN wiki_pages pg ON pg.wiki_id=l.wiki_id
LEFT JOIN memory_synthesis_pages s ON s.bank_id=:p1 AND s.page_id=pg.id
WHERE l.bank_id=:p1 AND (s.page_id IS NULL OR pg.revision>s.revision)
GROUP BY l.wiki_id;
