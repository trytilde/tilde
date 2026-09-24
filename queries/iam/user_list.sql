--: Record(email?, display_name?)
--! run (after?, limit_count, search) : Record
SELECT u.id,u.issuer,u.subject,u.email,u.display_name,
 COALESCE((SELECT array_agg(m.group_id ORDER BY m.group_id) FROM iam_group_members m WHERE m.user_id=u.id),'{}'::TEXT[]) AS group_ids
FROM iam_users u WHERE (:after::UUID IS NULL OR u.id>:after)
 AND (:search='' OR strpos(lower(COALESCE(u.email,'')||' '||COALESCE(u.display_name,'')||' '||u.subject),lower(:search))>0)
ORDER BY u.id LIMIT :limit_count;
