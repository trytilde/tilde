SELECT id, endpoint_url AS "endpoint_url!", webhook_signing_key
FROM agents WHERE endpoint_url IS NOT NULL ORDER BY id;
