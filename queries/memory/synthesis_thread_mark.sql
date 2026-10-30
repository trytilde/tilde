--! run (p1, p2, p3, p4)
INSERT INTO memory_synthesis_threads(bank_id,thread_id,through_at,through_id) VALUES(:p1,:p2,:p3,:p4)
ON CONFLICT(bank_id,thread_id) DO UPDATE SET through_at=EXCLUDED.through_at,through_id=EXCLUDED.through_id;
