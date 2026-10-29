--! run (pb, pk?, pp?, pf?, pt?, pe, pi?, pl) : (id)
-- Live, current memories in banks pb with recall_text's filters, newest first.
SELECT m.id
FROM memories m JOIN memory_banks b ON b.id=m.bank_id
WHERE m.bank_id=ANY(:pb::UUID[]) AND m.deleted_at IS NULL
  AND (:pk::TEXT IS NULL OR m.kind=:pk)
  AND (:pp::TEXT IS NULL OR :pp='' OR m.path=:pp OR starts_with(m.path,:pp||'/'))
  AND ((:pf::TIMESTAMPTZ IS NULL AND :pt::TIMESTAMPTZ IS NULL) OR m.valid_during && tstzrange(:pf,:pt,'[]'))
  AND (:pe OR b.scope='shared' OR m.identity_id IS NULL OR m.identity_id=:pi::UUID)
  AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id)
ORDER BY m.created_at DESC, m.id DESC
LIMIT :pl;
