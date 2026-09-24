--: Record()

--! run (p1) : Record
SELECT s.user_id, COALESCE((SELECT array_agg(m.group_id) FROM iam_group_members m WHERE m.user_id=s.user_id),'{}'::TEXT[]) AS groups
FROM iam_sessions s WHERE s.token_hash=:p1 AND s.expires_at>NOW();
