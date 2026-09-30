-- Arrays cannot carry NULL elements: `inline` says which rows keep their text, and an empty
-- object key means the file lives only in Postgres.
--! run (version_id, paths, media_types, sizes, digests, executables, inline, contents, object_keys)
INSERT INTO skill_files(version_id,path,media_type,size_bytes,sha256,executable,content,object_key)
SELECT :version_id,f.path,f.media_type,f.size,f.digest,f.executable,CASE WHEN f.inline THEN f.content END,NULLIF(f.object_key,'')
FROM UNNEST(:paths::TEXT[],:media_types::TEXT[],:sizes::BIGINT[],:digests::BYTEA[],:executables::BOOLEAN[],:inline::BOOLEAN[],:contents::TEXT[],:object_keys::TEXT[])
 AS f(path,media_type,size,digest,executable,inline,content,object_key);
