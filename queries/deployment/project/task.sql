--! run (p1, p2, p3, p4, p5, p6?, p7)
INSERT INTO chat_tasks(id,thread_id,agent_id,title,status,goal_id,blocked_reason) VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7) ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status,blocked_reason=EXCLUDED.blocked_reason;
