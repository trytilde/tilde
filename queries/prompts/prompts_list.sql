--: Record(latest_id?)

--! run (p1) : Record
SELECT p.id,p.agent_id,p.name,p.created_at,
 (SELECT v.id FROM prompt_versions v WHERE v.prompt_id=p.id ORDER BY v.number DESC LIMIT 1) AS latest_id
FROM prompts p WHERE p.agent_id=:p1 ORDER BY p.name;
