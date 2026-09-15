SELECT id, name, concurrency_policy AS "concurrency_policy: _", avatar_seed, avatar_key, paused, endpoint_url, created_at, updated_at, capabilities AS "capabilities!: Json<Capabilities>"
FROM agents
WHERE deleted_at IS NULL AND ($1::TIMESTAMPTZ IS NULL OR (created_at, id) < ($1, $2))
AND ($4 = '' OR strpos(lower(name), lower($4)) > 0 OR id::text = $4)
ORDER BY created_at DESC, id DESC
LIMIT $3;
