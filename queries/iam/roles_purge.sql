-- Deleting a resource takes its roles, statements and memberships with it.
--! run (kind, resource_id)
DELETE FROM iam_roles WHERE id IN (SELECT role_id FROM iam_role_statements WHERE resource_kind=:kind AND resource_id=:resource_id);
