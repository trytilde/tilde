-- Natural keys declarative apply (`ApplyService.Apply`) upserts by. An agent's key is its stable
-- identity in code; a routine is its agent's routine of that name. Existing routines that share
-- an agent and name are told apart by a numbered suffix first.
ALTER TABLE agents ADD COLUMN key TEXT CHECK (key ~ '^[a-z0-9][a-z0-9-]{0,63}$');
CREATE UNIQUE INDEX agents_key ON agents(key) WHERE deleted_at IS NULL;
UPDATE routines r SET name = left(r.name, 190) || ' (' || d.n || ')'
FROM (SELECT id, row_number() OVER (PARTITION BY agent_id, name ORDER BY created_at, id) AS n FROM routines) d
WHERE d.id = r.id AND d.n > 1;
CREATE UNIQUE INDEX routines_agent_name ON routines(agent_id, name);
