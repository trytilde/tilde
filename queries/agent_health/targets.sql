SELECT id, endpoint_url AS "endpoint_url!", webhook_signing_key
FROM agents WHERE deleted_at IS NULL AND endpoint_url IS NOT NULL ORDER BY id;
