--: Record(revoked_at?, created_by_user_id?)
-- Non-administrators see only the keys they created.
--! run (after?, limit_count, every, owner_user?) : Record
SELECT k.id,k.name,k.prefix,k.created_at,k.revoked_at,k.created_by_user_id,
 COALESCE((SELECT array_agg(r.id ORDER BY r.created_at,r.id) FROM iam_role_members m JOIN iam_roles r ON r.id=m.role_id WHERE m.api_key_id=k.id),'{}'::TEXT[]) AS role_ids,
 COALESCE((SELECT array_agg(r.name ORDER BY r.created_at,r.id) FROM iam_role_members m JOIN iam_roles r ON r.id=m.role_id WHERE m.api_key_id=k.id),'{}'::TEXT[]) AS role_names,
 COALESCE((SELECT array_agg(r.description ORDER BY r.created_at,r.id) FROM iam_role_members m JOIN iam_roles r ON r.id=m.role_id WHERE m.api_key_id=k.id),'{}'::TEXT[]) AS role_descriptions
FROM iam_api_keys k WHERE (:after::UUID IS NULL OR k.id>:after) AND (:every OR k.created_by_user_id=:owner_user)
ORDER BY k.id LIMIT :limit_count;
