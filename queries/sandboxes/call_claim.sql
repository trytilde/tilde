--: Record()

--! run (p1) : Record
-- Several processes may hold a Connect stream for one sandbox; each pending call goes to one.
UPDATE sandbox_calls SET status='delivered' WHERE id IN (
  SELECT id FROM sandbox_calls WHERE sandbox_id=:p1 AND status='pending' ORDER BY created_at FOR UPDATE SKIP LOCKED
) RETURNING id,operation,input_json;
