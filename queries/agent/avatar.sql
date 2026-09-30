--: Record(avatar_key?)

--! run (p1, p2) : Record
UPDATE agents SET avatar_key = :p2, updated_at = NOW()
WHERE id = :p1 AND deleted_at IS NULL
RETURNING id, name, description, concurrency_policy AS concurrency_policy, avatar_seed, avatar_key, paused, created_at, updated_at,
          capabilities AS capabilities;
