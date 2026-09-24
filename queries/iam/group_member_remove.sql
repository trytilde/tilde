--! run (group_id, user_id)
DELETE FROM iam_group_members WHERE group_id=:group_id AND user_id=:user_id;
