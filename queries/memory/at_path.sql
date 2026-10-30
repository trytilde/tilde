--! run (p1, p2, p3?, p4, p5) : (id)
-- At most p5 current memories exactly at path p2 in banks p1, newest first, with recall_text's
-- person filter.
SELECT m.id
FROM current_memories m
WHERE m.bank_id=ANY(:p1::UUID[]) AND m.path=:p2
  AND (:p4 OR m.scope='shared' OR m.identity_id IS NULL OR m.identity_id=:p3::UUID)
ORDER BY m.created_at DESC, m.id DESC
LIMIT :p5;
