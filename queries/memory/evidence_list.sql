--: Record(thread_id?, agent_id?, message_id?, page_id?, revision?, wiki_id?, wiki_name?, path?)

--! run (p1) : Record
-- What memories p1 were derived from: a message with its conversation's agent, or a page revision
-- with the page's wiki and path now (then, for a deleted page).
SELECT e.memory_id,e.thread_id,t.primary_agent_id AS agent_id,e.message_id,e.page_id,e.revision,
  COALESCE(pg.wiki_id,r.wiki_id) AS wiki_id,w.name AS wiki_name,COALESCE(pg.path,r.path) AS path
FROM memory_evidence e LEFT JOIN chat_threads t ON t.id=e.thread_id
LEFT JOIN wiki_page_revisions r ON r.page_id=e.page_id AND r.revision=e.revision
LEFT JOIN wiki_pages pg ON pg.id=e.page_id
LEFT JOIN wikis w ON w.id=COALESCE(pg.wiki_id,r.wiki_id)
WHERE e.memory_id=ANY(:p1::UUID[]) ORDER BY e.id;
