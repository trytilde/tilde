UPDATE agents
SET name = COALESCE($2, name),
    endpoint_url = CASE WHEN $3 THEN $4 ELSE endpoint_url END,
    capabilities = COALESCE($5, capabilities),
    updated_at = NOW()
WHERE id = $1
RETURNING id, name, endpoint_url, created_at, updated_at, capabilities AS "capabilities!: Json<Capabilities>";
