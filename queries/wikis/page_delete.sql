--! run (p1, p2) : (id)
-- Deletes the page only at the revision the writer read; returns no row otherwise.
DELETE FROM wiki_pages WHERE id=:p1 AND revision=:p2
RETURNING id;
