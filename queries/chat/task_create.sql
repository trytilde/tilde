INSERT INTO chat_tasks(id,thread_id,agent_id,title,goal_id) VALUES($1,$2,$3,$4,$5) ON CONFLICT(id) DO NOTHING;
