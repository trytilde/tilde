-- Inserts nothing for an unknown principal; re-assigning an existing role is a no-op that
-- still returns the row.
--! run (role_id, group_id?, user_id?, api_key_id?, by_user?)
INSERT INTO iam_role_members(role_id,group_id,user_id,api_key_id,granted_by_user_id)
SELECT :role_id,:group_id,:user_id,:api_key_id,:by_user
WHERE (:group_id::TEXT IS NULL OR EXISTS(SELECT 1 FROM iam_groups WHERE id=:group_id))
 AND (:user_id::UUID IS NULL OR EXISTS(SELECT 1 FROM iam_users WHERE id=:user_id))
 AND (:api_key_id::UUID IS NULL OR EXISTS(SELECT 1 FROM iam_api_keys WHERE id=:api_key_id AND revoked_at IS NULL))
ON CONFLICT(role_id,principal) DO UPDATE SET granted_by_user_id=iam_role_members.granted_by_user_id RETURNING principal;
