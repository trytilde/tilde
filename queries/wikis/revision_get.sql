--: Record(agent_id?, thread_id?)

--! run (p1, p2) : Record
SELECT r.page_id,r.wiki_id,r.revision,r.operation,r.path,r.title,r.content,r.agent_id,r.thread_id,r.at
FROM wiki_page_revisions r WHERE r.page_id=:p1 AND r.revision=:p2;
