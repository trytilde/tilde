--! run (p1) : (page_id, wiki_id, revision, operation, path)
-- The revision a write made, found by its call id: a retried call gets the same result.
SELECT r.page_id,r.wiki_id,r.revision,r.operation,r.path FROM wiki_page_revisions r WHERE r.call_id=:p1;
