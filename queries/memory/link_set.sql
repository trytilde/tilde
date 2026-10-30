--! run (p1, p2, p3)
-- A link no longer read-write stops retaining.
INSERT INTO agent_memory_banks(agent_id,bank_id,access) VALUES(:p1,:p2,:p3)
ON CONFLICT(agent_id,bank_id) DO UPDATE SET access=EXCLUDED.access,
  auto_retain=agent_memory_banks.auto_retain AND EXCLUDED.access='read_write';
