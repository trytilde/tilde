--! run (p1, p2)
INSERT INTO chat_users(id,name) VALUES(:p1,:p2) ON CONFLICT(id) DO NOTHING;
