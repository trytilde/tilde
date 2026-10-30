--! run (p1) : (agent_id)
-- The agents whose conversations feed bank p1.
SELECT agent_id FROM agent_memory_banks WHERE bank_id=:p1 AND auto_retain ORDER BY agent_id;
