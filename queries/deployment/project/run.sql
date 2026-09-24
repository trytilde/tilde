--! run (p1, p2, p3, p4, p5, p6?, p7)
INSERT INTO chat_runs(id,thread_id,agent_id,objective,status,goal_id,idempotency_key) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7) ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status;
