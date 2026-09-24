-- Roles with a statement on exactly this resource (the nil id lists installation-wide roles).
--! run (kind, resource_id)
SELECT r.id,r.name,r.description FROM iam_roles r
WHERE EXISTS(SELECT 1 FROM iam_role_statements s WHERE s.role_id=r.id AND s.resource_kind=:kind AND s.resource_id=:resource_id)
ORDER BY r.created_at,r.id;
