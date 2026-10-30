--: Record(agent_id?)

--! run (p1?, p2?) : Record
-- One wiki (p1) or all, optionally searched by name (p2), with the agents linked to it.
SELECT w.id,w.name,w.description,w.agent_id,w.created_at,
  -- The linked agents and, in the same order, each one's access.
  COALESCE((SELECT array_agg(l.agent_id ORDER BY l.agent_id) FROM agent_wikis l WHERE l.wiki_id=w.id), '{}')::UUID[] AS agent_ids,
  COALESCE((SELECT array_agg(l.access ORDER BY l.agent_id) FROM agent_wikis l WHERE l.wiki_id=w.id), '{}')::TEXT[] AS link_access,
  (SELECT COUNT(*) FROM wiki_pages p WHERE p.wiki_id=w.id) AS page_count
FROM wikis w
WHERE (:p1::UUID IS NULL OR w.id=:p1) AND (:p2::TEXT IS NULL OR w.name ILIKE '%'||:p2||'%')
ORDER BY w.name;
