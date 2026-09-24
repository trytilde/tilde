-- Inserts nothing when the group or user does not exist.
--! run (group_id, user_id)
INSERT INTO iam_group_members(group_id,user_id)
SELECT g.id,u.id FROM iam_groups g, iam_users u WHERE g.id=:group_id AND u.id=:user_id
ON CONFLICT DO NOTHING RETURNING group_id;
