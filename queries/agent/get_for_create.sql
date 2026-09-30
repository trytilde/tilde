--: Record(avatar_key?)

--! run (p1) : Record
SELECT id, name, description, concurrency_policy, avatar_seed, avatar_key, paused, created_at, updated_at, capabilities AS capabilities FROM agents WHERE deleted_at IS NULL AND id = :p1;
