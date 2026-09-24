--: Record()
--! run (group_id, after?, limit_count) : Record
SELECT m.user_id,COALESCE(u.email,u.display_name,u.subject) AS label
FROM iam_group_members m JOIN iam_users u ON u.id=m.user_id
WHERE m.group_id=:group_id AND (:after::UUID IS NULL OR m.user_id>:after) ORDER BY m.user_id LIMIT :limit_count;
