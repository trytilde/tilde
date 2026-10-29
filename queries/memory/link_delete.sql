--! run (p1, p2)
DELETE FROM agent_memory_banks WHERE agent_id=:p1 AND bank_id=:p2;
