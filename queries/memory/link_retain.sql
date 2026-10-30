--! run (p1, p2, p3)
-- Only a read-write link retains.
UPDATE agent_memory_banks SET auto_retain=:p3 WHERE agent_id=:p1 AND bank_id=:p2 AND access='read_write';
