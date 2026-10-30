--! run (p1, p2) : (title, content)
SELECT title,content FROM wiki_page_revisions WHERE page_id=:p1 AND revision=:p2;
