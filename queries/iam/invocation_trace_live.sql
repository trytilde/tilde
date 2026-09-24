--: Record()

--! run (p1, p2, p3, p4, p5) : Record
SELECT EXISTS(SELECT 1 FROM chat_invocations
 WHERE id=:p1 AND agent_id=:p2 AND thread_id=:p3 AND run_id=:p4
 AND ((status='running' AND lease_expires_at>NOW() AND to_timestamp(:p5)>NOW())
 OR (status IN ('stopped','failed','canceled') AND ended_at IS NOT NULL
 AND ended_at + INTERVAL '5 minutes'>NOW() AND ended_at<=to_timestamp(:p5)))) AS live;
