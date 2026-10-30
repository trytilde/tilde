--! run (p1, p2, p3)
INSERT INTO agent_memory_banks(agent_id,bank_id,access) VALUES(:p1,:p2,:p3)
ON CONFLICT(agent_id,bank_id) DO UPDATE SET access=EXCLUDED.access;
