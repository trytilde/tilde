--: Record()

--! run (p1, p2) : Record
INSERT INTO chat_users(id,name) VALUES(:p1,:p2) RETURNING id,name;
