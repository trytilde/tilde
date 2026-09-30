--: Record(latest_id?)

-- Every skill in these groups, for the groups an agent is given whole.
--! run (source_ids) : Record
SELECT s.id,s.source_id,src.name AS source_name,src.kind AS source_kind,s.name,s.source_path,s.created_at,
 (SELECT v.id FROM skill_versions v WHERE v.skill_id=s.id ORDER BY v.number DESC LIMIT 1) AS latest_id
FROM skills s JOIN skill_sources src ON src.id=s.source_id WHERE s.source_id=ANY(:source_ids::UUID[]) ORDER BY s.name, s.id;
