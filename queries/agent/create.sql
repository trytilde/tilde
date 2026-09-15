INSERT INTO agents (id, name, endpoint_url, webhook_signing_key, concurrency_policy, capabilities, avatar_seed)
VALUES ($1, $2, $3, $4, $5, $6, $1)
ON CONFLICT (id) DO NOTHING
RETURNING id, name, concurrency_policy AS "concurrency_policy: _", avatar_seed, avatar_key, paused, endpoint_url, created_at, updated_at, capabilities AS "capabilities!: Json<Capabilities>";
