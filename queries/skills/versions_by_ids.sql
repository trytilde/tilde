--: Record(commit_sha?)

--! run (ids) : Record
SELECT id,skill_id,number,hash,description,message,commit_sha,created_at FROM skill_versions WHERE id=ANY(:ids::UUID[]);
