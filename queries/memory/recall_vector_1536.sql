--! run (pb, pv, pn, pk?, pp?, pf?, pt?, po?, pd?, pe, pi?, pl) : (id)
-- Nearest memories in banks pb to embedding pv among those embedded under settings version pn,
-- with recall_text's filters. One query per supported dimension so the partial index applies;
-- run after vector_session so filtered scans still return enough rows.
SELECT m.id
FROM current_memories m
WHERE m.bank_id=ANY(:pb::UUID[])
  AND m.embedding_dims=1536 AND m.embedding_version=:pn
  AND (:pk::TEXT IS NULL OR m.kind=:pk)
  AND (:pp::TEXT IS NULL OR :pp='' OR m.path=:pp OR starts_with(m.path,:pp||'/'))
  AND ((:pf::TIMESTAMPTZ IS NULL AND :pt::TIMESTAMPTZ IS NULL) OR m.valid_during && tstzrange(:pf,:pt,'[]'))
  AND (:po::TEXT IS NULL OR m.origin=:po) AND (:pd::UUID IS NULL OR m.identity_id=:pd)
  AND (:pe OR m.scope='shared' OR m.identity_id IS NULL OR m.identity_id=:pi::UUID)
ORDER BY m.embedding::halfvec(1536) <=> :pv::TEXT::halfvec(1536)
LIMIT :pl;
