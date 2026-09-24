--! run (ids, names)
INSERT INTO iam_groups(id,name,source) SELECT g.id,g.name,'external' FROM unnest(:ids::TEXT[],:names::TEXT[]) AS g(id,name)
ON CONFLICT(id) DO NOTHING;
