--: Record(hash?, description?)

-- Each skill of a source with its latest version's hash and description, for sync to compare.
--! run (source_id) : Record
SELECT s.id,s.name,v.hash,v.description
FROM skills s LEFT JOIN LATERAL (SELECT hash,description FROM skill_versions v WHERE v.skill_id=s.id ORDER BY v.number DESC LIMIT 1) v ON true
WHERE s.source_id=:source_id ORDER BY s.name;
