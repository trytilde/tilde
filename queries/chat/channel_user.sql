--! run (p1, p2)
INSERT INTO chat_users(id,name) VALUES(:p1,LEFT(:p2,200)) ON CONFLICT DO NOTHING;
