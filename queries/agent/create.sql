INSERT INTO agents (id, name, endpoint_url, webhook_signing_key, capabilities)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (id) DO NOTHING
RETURNING id, name, endpoint_url, created_at, updated_at, capabilities AS "capabilities!: Json<Capabilities>";
