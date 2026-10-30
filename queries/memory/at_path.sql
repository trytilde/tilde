--! run (p1, p2, p3?, p4, p5) : (id)
-- At most p5 live, current memories exactly at path p2 in banks p1, newest first, with recall_text's person
-- filter.
SELECT m.id
FROM memories m JOIN memory_banks b ON b.id=m.bank_id
WHERE m.bank_id=ANY(:p1::UUID[]) AND m.deleted_at IS NULL AND m.path=:p2
  AND (:p4 OR b.scope='shared' OR m.identity_id IS NULL OR m.identity_id=:p3::UUID)
  AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id)
ORDER BY m.created_at DESC, m.id DESC
LIMIT :p5;
