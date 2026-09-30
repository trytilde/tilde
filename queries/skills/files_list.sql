--: Record(content?, object_key?)

--! run (version_ids) : Record
SELECT version_id,path,media_type,size_bytes,sha256,executable,content,object_key FROM skill_files WHERE version_id=ANY(:version_ids::UUID[]) ORDER BY version_id,path;
