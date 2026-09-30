--: Record(latest_id?)

--! run (agent) : Record
SELECT s.id,s.source_id,src.name AS source_name,src.kind AS source_kind,s.name,s.source_path,s.created_at,
 (SELECT v.id FROM skill_versions v WHERE v.skill_id=s.id ORDER BY v.number DESC LIMIT 1) AS latest_id
FROM agent_skills a JOIN skills s ON s.id=a.skill_id JOIN skill_sources src ON src.id=s.source_id WHERE a.agent_id=:agent ORDER BY s.name;
