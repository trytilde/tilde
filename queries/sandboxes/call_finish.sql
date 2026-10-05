--! run (p1, p2, p3, p4, p5)
-- Only the sandbox the call was delivered to completes it, once.
UPDATE sandbox_calls SET status=:p3,output_json=:p4,error=:p5 WHERE id=:p1 AND sandbox_id=:p2 AND status='delivered';
