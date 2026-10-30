--! run (p1, p2)
INSERT INTO memory_bank_wikis(bank_id,wiki_id) VALUES(:p1,:p2) ON CONFLICT DO NOTHING;
