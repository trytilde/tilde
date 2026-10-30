--: Record(thread_id?, agent_id?, page_id?, revision?, wiki_id?, path?)

--! run (p1, p2) : Record
-- Bank p1's latest p2 batches, with where each came from: a conversation and its agent, or a page
-- revision with the page's wiki and path now (then, for a deleted page).
SELECT b.id,b.thread_id,t.primary_agent_id AS agent_id,b.page_id,b.revision,
  COALESCE(pg.wiki_id,r.wiki_id) AS wiki_id,COALESCE(pg.path,r.path) AS path,
  b.status,b.written,b.superseded,b.forgotten,b.dropped,b.notes,b.at
FROM memory_synthesis_batches b LEFT JOIN chat_threads t ON t.id=b.thread_id
LEFT JOIN wiki_page_revisions r ON r.page_id=b.page_id AND r.revision=b.revision
LEFT JOIN wiki_pages pg ON pg.id=b.page_id
WHERE b.bank_id=:p1 ORDER BY b.at DESC LIMIT :p2;
