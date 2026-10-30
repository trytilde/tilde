--! run (p1, p2)
DELETE FROM agent_wikis WHERE agent_id=:p1 AND wiki_id=:p2;
