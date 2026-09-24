--! run (role_id)
SELECT resource_kind,resource_id,action FROM iam_role_statements WHERE role_id=:role_id ORDER BY resource_kind,resource_id,action;
