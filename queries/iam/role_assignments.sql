--: Record(group_id?, user_id?, api_key_id?, label?)
-- Every principal holding a role with a statement on the resource, one row per role held.
--! run (kind, resource_id) : Record
SELECT m.role_id,r.name,r.description,m.group_id,m.user_id,m.api_key_id,m.created_at,
 COALESCE(gr.name,u.email,u.display_name,u.subject,k.name) AS label
FROM iam_role_members m JOIN iam_roles r ON r.id=m.role_id
LEFT JOIN iam_groups gr ON gr.id=m.group_id LEFT JOIN iam_users u ON u.id=m.user_id LEFT JOIN iam_api_keys k ON k.id=m.api_key_id
WHERE EXISTS(SELECT 1 FROM iam_role_statements s WHERE s.role_id=m.role_id AND s.resource_kind=:kind AND s.resource_id=:resource_id)
ORDER BY m.principal,r.created_at,r.id;
