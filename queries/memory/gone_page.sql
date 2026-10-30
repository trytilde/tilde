--! run (p1) : (page_id, revision)
-- A page bank p1 synthesized that no longer exists. (Unlinking a wiki drops its pages' watermarks,
-- so their memories stay.)
SELECT s.page_id,s.revision FROM memory_synthesis_pages s
WHERE s.bank_id=:p1 AND NOT EXISTS (SELECT 1 FROM wiki_pages pg WHERE pg.id=s.page_id) LIMIT 1;
