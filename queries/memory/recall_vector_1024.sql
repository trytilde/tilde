--! run (pb, pv, pn, pk?, pp?, pf?, pt?, pe, pi?, pl) : (id)
-- Nearest memories in banks pb to embedding pv among those embedded under settings version pn,
-- with recall_text's filters. One query per supported dimension so the partial index applies;
-- run after vector_session so filtered scans still return enough rows.
SELECT m.id
FROM memories m JOIN memory_banks b ON b.id=m.bank_id
WHERE m.bank_id=ANY(:pb::UUID[]) AND m.deleted_at IS NULL
  AND m.embedding_dims=1024 AND m.embedding_version=:pn
  AND (:pk::TEXT IS NULL OR m.kind=:pk)
  AND (:pp::TEXT IS NULL OR :pp='' OR m.path=:pp OR starts_with(m.path,:pp||'/'))
  AND ((:pf::TIMESTAMPTZ IS NULL AND :pt::TIMESTAMPTZ IS NULL) OR m.valid_during && tstzrange(:pf,:pt,'[]'))
  AND (:pe OR b.scope='shared' OR m.identity_id IS NULL OR m.identity_id=:pi::UUID)
  AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id)
ORDER BY m.embedding::halfvec(1024) <=> :pv::TEXT::halfvec(1024)
LIMIT :pl;
