--! run (user_id)
DELETE FROM iam_sessions WHERE user_id=:user_id;
