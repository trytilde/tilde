--! run (p1) : (wiki_id, name)
SELECT l.wiki_id,w.name FROM memory_bank_wikis l JOIN wikis w ON w.id=l.wiki_id WHERE l.bank_id=:p1 ORDER BY w.name;
