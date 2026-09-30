--: Record(avatar_key?)

--! run (p1, p2?, p3?, p4?, description?) : Record
UPDATE agents
SET name = COALESCE(:p2, name),
    description = COALESCE(:description, description),
    concurrency_policy = COALESCE(:p3, concurrency_policy),
    capabilities = COALESCE(:p4, capabilities),
    updated_at = NOW()
WHERE deleted_at IS NULL AND id = :p1
RETURNING id, name, description, concurrency_policy AS concurrency_policy, avatar_seed, avatar_key, paused, created_at, updated_at, capabilities AS capabilities;
