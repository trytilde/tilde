--: Record(catalog_group?, repository_url?, git_ref?, commit_sha?, sync_error?)

--! run (connection_id) : Record
SELECT src.id,src.slug,src.name,src.kind,src.catalog_group,src.repository_url,src.git_ref,src.git_path,src.commit_sha,src.sync_error,src.created_at,
 (SELECT COUNT(*) FROM skills s WHERE s.source_id=src.id)::INTEGER AS skill_count
FROM connection_skill_sources l JOIN skill_sources src ON src.id=l.source_id WHERE l.connection_id=:connection_id
ORDER BY src.name, src.id;
