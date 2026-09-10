INSERT INTO iam_sessions(token_hash,user_id,expires_at) VALUES($1,$2,NOW()+INTERVAL '8 hours');
