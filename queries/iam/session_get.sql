SELECT user_id FROM iam_sessions WHERE token_hash=$1 AND expires_at>NOW();
