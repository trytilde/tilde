INSERT INTO chat_users(id,name) VALUES($1,$2) ON CONFLICT(id) DO NOTHING;
