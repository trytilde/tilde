--: Record(commit_sha?)

--! run (skill_id) : Record
SELECT id,skill_id,number,hash,description,message,commit_sha,created_at FROM skill_versions WHERE skill_id=:skill_id ORDER BY number DESC LIMIT 1;
