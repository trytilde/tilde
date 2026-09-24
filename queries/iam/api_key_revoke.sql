--! run (id, every, owner_user?)
UPDATE iam_api_keys SET revoked_at=COALESCE(revoked_at,NOW()) WHERE id=:id AND (:every OR created_by_user_id=:owner_user);
