--: Record()

--! run (p1) : Record
-- The wikis linked to an agent; `own` marks the wiki that is the agent's.
SELECT l.agent_id,l.wiki_id,w.name AS wiki_name,w.description,l.access,COALESCE(w.agent_id=l.agent_id,FALSE) AS own
FROM agent_wikis l JOIN wikis w ON w.id=l.wiki_id
WHERE l.agent_id=:p1
ORDER BY w.name;
