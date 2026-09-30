--: Record()

--! run (id, skill_id, hash, description, message, commit_sha?) : Record
INSERT INTO skill_versions(id,skill_id,number,hash,description,message,commit_sha)
SELECT :id,:skill_id,COALESCE(MAX(number),0)+1,:hash,:description,:message,:commit_sha FROM skill_versions WHERE skill_id=:skill_id
RETURNING id;
