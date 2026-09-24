--! run (user_id, ids)
INSERT INTO iam_group_members(group_id,user_id) SELECT g,:user_id FROM unnest(:ids::TEXT[]) g ON CONFLICT DO NOTHING;
