--! run (role_id, kind, resource_id, action)
INSERT INTO iam_role_statements(role_id,resource_kind,resource_id,action) VALUES(:role_id,:kind,:resource_id,:action) ON CONFLICT DO NOTHING;
