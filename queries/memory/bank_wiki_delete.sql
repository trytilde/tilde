--! run (p1, p2)
-- Wiki p2 stops feeding bank p1. Its pages' watermarks go, so memories already synthesized from
-- them stay; linking it again reads every page anew.
WITH unmarked AS (DELETE FROM memory_synthesis_pages s USING wiki_pages pg
  WHERE s.bank_id=:p1 AND pg.id=s.page_id AND pg.wiki_id=:p2)
DELETE FROM memory_bank_wikis WHERE bank_id=:p1 AND wiki_id=:p2;
