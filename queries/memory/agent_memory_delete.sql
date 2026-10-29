--! run (p1)
-- A deleted agent's own banks (their memories and roles go with them) and its links.
WITH links AS (DELETE FROM agent_memory_banks WHERE agent_id=:p1)
DELETE FROM memory_banks WHERE agent_id=:p1;
