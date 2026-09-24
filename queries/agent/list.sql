--: Record(avatar_key?)

-- Management lists pass the caller so invisible agents never reach a page; runtime lists pass is_admin.
--! run (p1?, p2?, p3, p4, is_admin, caller_user?, caller_key?, caller_groups) : Record
SELECT id, name, concurrency_policy AS concurrency_policy, avatar_seed, avatar_key, paused, created_at, updated_at, capabilities AS capabilities
FROM agents
WHERE deleted_at IS NULL AND (:p1::TIMESTAMPTZ IS NULL OR (created_at, id) < (:p1, :p2))
AND (:p4 = '' OR strpos(lower(name), lower(:p4)) > 0 OR id::text = :p4)
AND iam_held('agent', id, ARRAY['view']::TEXT[], :is_admin, :caller_user, :caller_key, :caller_groups::TEXT[])
ORDER BY created_at DESC, id DESC
LIMIT :p3;
