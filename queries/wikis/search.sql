--! run (p1, p2, p3)
-- Pages in the wikis (p1) matching any of the query's words (each stemmed once), best first,
-- with a short excerpt around the matches, each marked by U+E000 and U+E001 (private-use
-- characters, so Markdown in the content cannot be mistaken for a mark).
WITH q AS (
  SELECT replace(plainto_tsquery('english', :p2)::TEXT, ' & ', ' | ')::tsquery AS tsq
)
SELECT p.id,p.wiki_id,w.name AS wiki_name,p.path,p.title,p.revision,p.updated_at,
  ts_rank_cd(p.search, q.tsq) AS rank,
  ts_headline('english', p.content, q.tsq, 'MaxFragments=2, MaxWords=24, MinWords=8, StartSel=, StopSel=') AS excerpt
FROM wiki_pages p JOIN wikis w ON w.id=p.wiki_id, q
WHERE p.wiki_id = ANY(:p1) AND numnode(q.tsq) > 0 AND p.search @@ q.tsq
ORDER BY rank DESC, p.path
LIMIT :p3;
