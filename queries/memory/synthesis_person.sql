--! run (p1) : (identity)
-- The person conversation p1 is with: its only active human, as a root identity, when verified
-- or attested. An inbound channel address alone is only a claim.
WITH humans AS (
  SELECT p.user_id AS id FROM chat_participants p WHERE p.thread_id=:p1 AND p.active AND p.user_id IS NOT NULL
)
SELECT COALESCE(u.root_identity_id,u.id) AS identity FROM chat_users u
LEFT JOIN chat_channel_identities i ON i.id=u.id
WHERE (SELECT COUNT(*) FROM humans)=1 AND u.id=(SELECT id FROM humans)
  AND (i.id IS NULL OR i.verified_at IS NOT NULL OR u.attested_at IS NOT NULL);
