--: Record(avatar_key?)

--! run (p1, p2, p3, p4, description) : Record
INSERT INTO agents (id, name, description, concurrency_policy, capabilities, avatar_seed)
VALUES (:p1, :p2, :description, :p3, :p4, :p1)
ON CONFLICT (id) DO NOTHING
RETURNING id, name, description, concurrency_policy AS concurrency_policy, avatar_seed, avatar_key, paused, created_at, updated_at, capabilities AS capabilities;
