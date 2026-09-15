UPDATE agents SET avatar_key = $2, updated_at = NOW()
WHERE id = $1 AND deleted_at IS NULL
RETURNING id, name, concurrency_policy AS "concurrency_policy: _", avatar_seed, avatar_key, paused, endpoint_url, created_at, updated_at,
          capabilities AS "capabilities!: Json<Capabilities>";
