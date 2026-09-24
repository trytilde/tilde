--: Record()
--! run (kind, action, ids, is_admin, caller_user?, caller_key?, caller_groups) : Record
SELECT r.id,
 iam_held(:kind,r.id,ARRAY[:action]::TEXT[],:is_admin,:caller_user,:caller_key,:caller_groups::TEXT[]) AS held,
 iam_held(:kind,r.id,ARRAY['view']::TEXT[],:is_admin,:caller_user,:caller_key,:caller_groups::TEXT[]) AS visible
FROM unnest(:ids::UUID[]) AS r(id);
