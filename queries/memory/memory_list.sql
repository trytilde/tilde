--! run (pb, pk?, pp?, pf?, pt?, po?, pd?, pe, pi?, pc?, pl) : (id)
-- Current memories in banks pb with recall_text's filters, newest first, after memory pc when
-- given (the previous page's last).
SELECT m.id
FROM current_memories m
WHERE m.bank_id=ANY(:pb::UUID[])
  AND (:pk::TEXT IS NULL OR m.kind=:pk)
  AND (:pp::TEXT IS NULL OR :pp='' OR m.path=:pp OR starts_with(m.path,:pp||'/'))
  AND ((:pf::TIMESTAMPTZ IS NULL AND :pt::TIMESTAMPTZ IS NULL) OR m.valid_during && tstzrange(:pf,:pt,'[]'))
  AND (:po::TEXT IS NULL OR m.origin=:po) AND (:pd::UUID IS NULL OR m.identity_id=:pd)
  AND (:pe OR m.scope='shared' OR m.identity_id IS NULL OR m.identity_id=:pi::UUID)
  AND (:pc::UUID IS NULL OR (m.created_at, m.id) < (SELECT c.created_at, c.id FROM memories c WHERE c.id=:pc))
ORDER BY m.created_at DESC, m.id DESC
LIMIT :pl;
