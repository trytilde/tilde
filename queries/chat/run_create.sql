--: Record()

--! run (p1, p2, p3, p4, p5?, p6) : Record
INSERT INTO chat_runs(id,thread_id,agent_id,objective,goal_id,idempotency_key) VALUES(:p1,:p2,:p3,:p4,:p5,:p6) ON CONFLICT(thread_id,agent_id,idempotency_key) DO NOTHING RETURNING id;
