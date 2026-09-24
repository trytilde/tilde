--: Record()
--! run (after?, limit_count, search) : Record
SELECT g.id,g.name,g.source,g.created_at,(SELECT COUNT(*) FROM iam_group_members m WHERE m.group_id=g.id) AS member_count
FROM iam_groups g WHERE (:after::TEXT IS NULL OR g.id>:after)
 AND (:search='' OR strpos(lower(g.id||' '||g.name),lower(:search))>0)
ORDER BY g.id LIMIT :limit_count;
