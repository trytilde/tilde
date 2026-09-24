--: Record()

--! run (p1, p2, p3, p4) : Record
SELECT i.status, r.status AS run_status, i.lease_expires_at > NOW() AS lease_live
FROM chat_invocations i JOIN chat_runs r ON r.id = i.run_id
WHERE i.id = :p1 AND i.agent_id = :p2 AND i.thread_id = :p3 AND i.run_id = :p4;
