--: Record()

--! run (p1?, p2?, p3?) : Record
-- A page by id (p1), or by its wiki (p2) and path (p3).
SELECT p.id,p.wiki_id,w.name AS wiki_name,p.path,p.title,p.content,p.revision,p.created_at,p.updated_at
FROM wiki_pages p JOIN wikis w ON w.id=p.wiki_id
WHERE p.id=:p1 OR (p.wiki_id=:p2 AND p.path=:p3);
