--! run (p1, p2) : (id)
-- Bank p1's current memories derived from page p2, newest first.
SELECT DISTINCT m.id,m.created_at FROM current_memories m JOIN memory_evidence e ON e.memory_id=m.id
WHERE m.bank_id=:p1 AND e.page_id=:p2 ORDER BY m.created_at DESC LIMIT 50;
