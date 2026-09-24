--: Record(revoked_at?, created_by_user_id?)
--! run (id, name, prefix, token_hash, user_id) : Record
INSERT INTO iam_api_keys(id,name,prefix,token_hash,created_by_user_id)
VALUES(:id,:name,:prefix,:token_hash,:user_id)
RETURNING id,name,prefix,created_at,revoked_at,created_by_user_id;
