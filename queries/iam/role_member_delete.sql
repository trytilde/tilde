--! run (role_id, group_id?, user_id?, api_key_id?)
DELETE FROM iam_role_members WHERE role_id=:role_id AND (group_id=:group_id OR user_id=:user_id OR api_key_id=:api_key_id);
