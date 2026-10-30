--! run (p1, p2, p3)
-- Bank p1 has synthesized page p2 through revision p3. A batch that finishes after the page's wiki
-- was unlinked from the bank marks nothing, so no watermark outlives the link (a deleted page is
-- still marked: cleaning it up needs the watermark).
INSERT INTO memory_synthesis_pages(bank_id,page_id,revision)
SELECT :p1,:p2,:p3 WHERE NOT EXISTS (SELECT 1 FROM wiki_pages pg WHERE pg.id=:p2
  AND NOT EXISTS (SELECT 1 FROM memory_bank_wikis l WHERE l.bank_id=:p1 AND l.wiki_id=pg.wiki_id))
ON CONFLICT(bank_id,page_id) DO UPDATE SET revision=EXCLUDED.revision;
