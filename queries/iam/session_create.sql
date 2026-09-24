--! run (p1, p2)
INSERT INTO iam_sessions(token_hash,user_id,expires_at) VALUES(:p1,:p2,NOW()+INTERVAL '8 hours');
