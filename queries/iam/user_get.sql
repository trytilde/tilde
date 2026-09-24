--: Record(email?, display_name?)
--! run (id) : Record
SELECT u.id,u.issuer,u.subject,u.email,u.display_name,
 COALESCE((SELECT array_agg(m.group_id ORDER BY m.group_id) FROM iam_group_members m WHERE m.user_id=u.id),'{}'::TEXT[]) AS group_ids
FROM iam_users u WHERE u.id=:id;
