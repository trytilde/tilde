--: Record()
--! run (id) : Record
SELECT g.id,g.name,g.source,g.created_at,(SELECT COUNT(*) FROM iam_group_members m WHERE m.group_id=g.id) AS member_count
FROM iam_groups g WHERE g.id=:id;
