--: Record(agent_id?, thread_id?)

--! run (p1) : Record
-- A page's history, newest first, without content.
SELECT r.revision,r.operation,r.path,r.title,r.agent_id,r.thread_id,r.at
FROM wiki_page_revisions r WHERE r.page_id=:p1
ORDER BY r.revision DESC;
