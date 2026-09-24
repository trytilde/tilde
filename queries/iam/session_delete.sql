--! run (p1)
DELETE FROM iam_sessions WHERE token_hash=:p1;
