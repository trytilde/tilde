--! run (p1, p2, p3, p4)
-- Lines of current pages in the wikis (p1) matching the regular expression (p2), case-insensitive
-- when p3, in path and line order, each cut to 500 characters. Runs under grep_session's
-- statement timeout.
SELECT p.wiki_id,w.name AS wiki_name,p.path,l.n AS line_number,left(l.line, 500) AS line,
  length(l.line) > 500 AS truncated
FROM wiki_pages p JOIN wikis w ON w.id=p.wiki_id,
  LATERAL unnest(string_to_array(p.content, E'\n')) WITH ORDINALITY AS l(line, n)
WHERE p.wiki_id = ANY(:p1) AND CASE WHEN :p3 THEN l.line ~* :p2 ELSE l.line ~ :p2 END
ORDER BY p.path, l.n
LIMIT :p4;
