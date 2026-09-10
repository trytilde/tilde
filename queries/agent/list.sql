SELECT id, name, endpoint_url, created_at, updated_at, capabilities AS "capabilities!: Json<Capabilities>"
FROM agents
WHERE ($1::TIMESTAMPTZ IS NULL OR (created_at, id) < ($1, $2))
ORDER BY created_at DESC, id DESC
LIMIT $3;
