--! run (p1, p2, p3, p4, p5) : (revision)
-- Changes the page only at the revision the writer read; returns no row otherwise.
UPDATE wiki_pages SET path=:p3,title=:p4,content=:p5,revision=revision+1,updated_at=NOW()
WHERE id=:p1 AND revision=:p2
RETURNING revision;
