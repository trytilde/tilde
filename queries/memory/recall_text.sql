--! run (pb, pq, pk?, pp?, pf?, pt?, pe, pi?, pl) : (id)
-- Full-text candidates in banks pb for query pq, best first; any of its words matches.
-- Filters: kind pk, path pp and below, overlapping [pf, pt]. In per-person banks only person
-- pi's memories and bank-wide ones count, unless pe (a person managing the bank) sees all.
-- The query's normalised terms are joined with OR as they are, without stemming them again.
WITH q AS (SELECT replace(plainto_tsquery('english', :pq)::TEXT, ' & ', ' | ')::tsquery AS q)
SELECT m.id
FROM memories m JOIN memory_banks b ON b.id=m.bank_id, q
WHERE m.bank_id=ANY(:pb::UUID[]) AND m.deleted_at IS NULL AND numnode(q.q) > 0 AND m.search @@ q.q
  AND (:pk::TEXT IS NULL OR m.kind=:pk)
  AND (:pp::TEXT IS NULL OR :pp='' OR m.path=:pp OR starts_with(m.path,:pp||'/'))
  AND ((:pf::TIMESTAMPTZ IS NULL AND :pt::TIMESTAMPTZ IS NULL) OR m.valid_during && tstzrange(:pf,:pt,'[]'))
  AND (:pe OR b.scope='shared' OR m.identity_id IS NULL OR m.identity_id=:pi::UUID)
  AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id)
ORDER BY ts_rank_cd(m.search, q.q) DESC, m.id
LIMIT :pl;
