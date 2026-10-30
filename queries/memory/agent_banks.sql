--: Record()

--! run (p1) : Record
-- The banks linked to an agent; `own` marks the bank that is the agent's.
SELECT l.agent_id,l.bank_id,b.name AS bank_name,b.description,l.access,b.scope,COALESCE(b.agent_id=l.agent_id,FALSE) AS own
FROM agent_memory_banks l JOIN memory_banks b ON b.id=l.bank_id
WHERE l.agent_id=:p1
ORDER BY b.name;
