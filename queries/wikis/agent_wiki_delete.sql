--! run (p1)
-- A deleted agent's own wikis (their pages go with them) and its links.
WITH links AS (DELETE FROM agent_wikis WHERE agent_id=:p1)
DELETE FROM wikis WHERE agent_id=:p1;
