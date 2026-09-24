--: Record()
--! run (token_hash) : Record
SELECT id FROM iam_api_keys WHERE token_hash=:token_hash AND revoked_at IS NULL;
