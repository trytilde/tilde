UPDATE agents
SET name = COALESCE($2, name),
    endpoint_url = CASE WHEN $3 THEN $4 ELSE endpoint_url END,
    concurrency_policy = COALESCE($5, concurrency_policy),
    capabilities = COALESCE($6, capabilities),
    updated_at = NOW()
WHERE deleted_at IS NULL AND id = $1
RETURNING id, name, concurrency_policy AS "concurrency_policy: _", avatar_seed, avatar_key, paused, endpoint_url, created_at, updated_at, capabilities AS "capabilities!: Json<Capabilities>";
