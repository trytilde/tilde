--! run (pb, pq, pk?, pp?, pf?, pt?, po?, pd?, pe, pi?, pl) : (id)
-- Full-text candidates in banks pb for query pq, best first; any of its words matches.
-- Filters: kind pk, path pp and below, overlapping [pf, pt], origin po, person pd. In per-person
-- banks only person pi's memories and bank-wide ones count, unless pe (a person managing the
-- bank) sees all. The query's normalised terms are joined with OR as they are, without stemming
-- them again.
WITH q AS (SELECT replace(plainto_tsquery('english', :pq)::TEXT, ' & ', ' | ')::tsquery AS q)
SELECT m.id
FROM current_memories m, q
WHERE m.bank_id=ANY(:pb::UUID[]) AND numnode(q.q) > 0 AND m.search @@ q.q
  AND (:pk::TEXT IS NULL OR m.kind=:pk)
  AND (:pp::TEXT IS NULL OR :pp='' OR m.path=:pp OR starts_with(m.path,:pp||'/'))
  AND ((:pf::TIMESTAMPTZ IS NULL AND :pt::TIMESTAMPTZ IS NULL) OR m.valid_during && tstzrange(:pf,:pt,'[]'))
  AND (:po::TEXT IS NULL OR m.origin=:po) AND (:pd::UUID IS NULL OR m.identity_id=:pd)
  AND (:pe OR m.scope='shared' OR m.identity_id IS NULL OR m.identity_id=:pi::UUID)
ORDER BY ts_rank_cd(m.search, q.q) DESC, m.id
LIMIT :pl;
