--: Record(commit_sha?)

--! run (id) : Record
SELECT id,skill_id,number,hash,description,message,commit_sha,created_at FROM skill_versions WHERE id=:id;
