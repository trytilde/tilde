INSERT INTO chat_threads(id,title,primary_agent_id) VALUES($1,$2,$3) ON CONFLICT(id) DO UPDATE SET title=EXCLUDED.title;
