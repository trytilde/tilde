--: Record(synthesized?)

--! run (p1, p2) : Record
-- The page of a wiki linked to bank p1 changed longest ago since it was last synthesized there,
-- and unchanged for five minutes (so rapid saves coalesce), with its content cut to p2 characters
-- (`truncated`) and the revision last synthesized, if any.
SELECT pg.id AS page_id,pg.wiki_id,w.name AS wiki_name,pg.revision,pg.path,pg.title,
  left(pg.content,:p2) AS content,length(pg.content)>:p2 AS truncated,s.revision AS synthesized
FROM memory_bank_wikis l JOIN wiki_pages pg ON pg.wiki_id=l.wiki_id JOIN wikis w ON w.id=l.wiki_id
LEFT JOIN memory_synthesis_pages s ON s.bank_id=:p1 AND s.page_id=pg.id
WHERE l.bank_id=:p1 AND (s.page_id IS NULL OR pg.revision>s.revision)
  AND pg.updated_at<NOW()-interval '5 minutes'
ORDER BY pg.updated_at LIMIT 1;
