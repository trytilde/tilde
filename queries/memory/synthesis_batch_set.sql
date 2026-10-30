--! run (p1, p2, p3, p4?, p5?, p6?, p7, p8, p9, p10, p11, p12, p13) : (id)
-- Records batch p1 (from source start p3) as p7, counting a failure its content caused (p8).
-- Nothing is returned when it was already applied or skipped, so a concurrent second application
-- rolls back.
INSERT INTO memory_synthesis_batches(id,bank_id,start_id,thread_id,page_id,revision,status,failures,written,superseded,forgotten,dropped,notes)
VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,CASE WHEN :p8::BOOLEAN THEN 1 ELSE 0 END,:p9,:p10,:p11,:p12,left(:p13,4000))
ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status,
  failures=memory_synthesis_batches.failures+EXCLUDED.failures,written=EXCLUDED.written,
  superseded=EXCLUDED.superseded,forgotten=EXCLUDED.forgotten,dropped=EXCLUDED.dropped,notes=EXCLUDED.notes,at=NOW()
WHERE memory_synthesis_batches.status='failed'
RETURNING id;
