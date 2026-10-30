--! run (p1, p2, p3, p4, p5)
-- Inserts nothing when the wiki has a page at the path.
INSERT INTO wiki_pages(id,wiki_id,path,title,content,revision) VALUES(:p1,:p2,:p3,:p4,:p5,1)
ON CONFLICT(wiki_id,path) DO NOTHING;
