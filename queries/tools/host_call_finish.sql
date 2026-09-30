--! run (p1, p2, p3, p4, p5, p6)
-- A verify's result is the account label (p6) rather than tool output.
UPDATE tool_host_calls SET status=:p3,output_json=CASE WHEN kind='verify' THEN :p6 ELSE :p4 END,error=:p5 WHERE id=:p1 AND tool_host_id=:p2 AND status='delivered';
