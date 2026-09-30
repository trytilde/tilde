--: Record(content?, object_key?)

--! run (version_id, path) : Record
SELECT version_id,path,media_type,size_bytes,sha256,executable,content,object_key FROM skill_files WHERE version_id=:version_id AND path=:path;
