--: Record(catalog_group?, repository_url?, git_ref?, commit_sha?, sync_error?)

--! run (agent) : Record
SELECT src.id,src.slug,src.name,src.kind,src.catalog_group,src.repository_url,src.git_ref,src.git_path,src.commit_sha,src.sync_error,src.created_at,
 (SELECT COUNT(*) FROM skills s WHERE s.source_id=src.id)::INTEGER AS skill_count
FROM agent_skill_sources a JOIN skill_sources src ON src.id=a.source_id WHERE a.agent_id=:agent ORDER BY src.name, src.id;
