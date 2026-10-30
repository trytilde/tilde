--! run (p1, p2?, p3)
-- A wiki's pages without their content, in path order, for its tree; with a prefix (p2), only
-- the page at it and those below. At most p3: callers ask for one more than they show to tell
-- whether more exist.
SELECT p.id,p.path,p.title,p.revision,p.updated_at
FROM wiki_pages p
WHERE p.wiki_id=:p1 AND (:p2::TEXT IS NULL OR p.path=:p2 OR starts_with(p.path, :p2 || '/'))
ORDER BY p.path
LIMIT :p3;
