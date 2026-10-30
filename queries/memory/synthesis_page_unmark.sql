--! run (p1, p2)
DELETE FROM memory_synthesis_pages WHERE bank_id=:p1 AND page_id=:p2;
