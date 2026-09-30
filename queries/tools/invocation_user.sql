--: Record(root_identity_id?)

--! run (p1, p2) : Record
-- The person an invocation acts for: the run's source identity, else the thread's only active
-- human. An inbound channel address is a claim until verified or attested, so an unverified
-- channel identity never unlocks personal tools.
WITH humans AS (
  SELECT p.user_id AS id FROM chat_participants p WHERE p.thread_id=:p2 AND p.active AND p.user_id IS NOT NULL
), chosen AS (
  SELECT COALESCE(
    (SELECT r.source_identity_id FROM chat_runs r WHERE r.id=:p1 AND r.thread_id=:p2),
    (SELECT id FROM humans WHERE (SELECT COUNT(*) FROM humans)=1 LIMIT 1)) AS id
)
SELECT u.id,u.root_identity_id FROM chat_users u JOIN chosen c ON c.id=u.id
LEFT JOIN chat_channel_identities i ON i.id=u.id
WHERE i.id IS NULL OR i.verified_at IS NOT NULL OR u.attested_at IS NOT NULL;
