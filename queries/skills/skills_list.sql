--: Record(latest_id?)

-- Skills, optionally of one source.
--! run (source_id?) : Record
SELECT s.id,s.source_id,src.name AS source_name,src.kind AS source_kind,s.name,s.source_path,s.created_at,
 (SELECT v.id FROM skill_versions v WHERE v.skill_id=s.id ORDER BY v.number DESC LIMIT 1) AS latest_id
FROM skills s JOIN skill_sources src ON src.id=s.source_id
WHERE src.kind<>'bundled' AND (:source_id::UUID IS NULL OR s.source_id=:source_id)
ORDER BY s.name, src.name, s.id;
