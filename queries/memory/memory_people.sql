--! run (p1) : ()
-- The people with current memories in bank p1 and how many each has. An identity is a root
-- identity or a lone user; a root identity is named after one of its users.
SELECT m.identity_id,
  COALESCE((SELECT u.name FROM chat_users u WHERE u.id=m.identity_id),
           (SELECT min(u.name) FROM chat_users u WHERE u.root_identity_id=m.identity_id), '') AS name,
  COUNT(*) AS count
FROM current_memories m WHERE m.bank_id=:p1 AND m.identity_id IS NOT NULL
GROUP BY m.identity_id ORDER BY 2, 1;
