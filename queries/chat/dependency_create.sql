--! run (p1, p2, p3, p4)
INSERT INTO chat_task_dependencies(thread_id,agent_id,task_id,dependency_id) VALUES(:p1,:p2,:p3,:p4);
