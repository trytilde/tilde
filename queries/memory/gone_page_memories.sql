--! run (p1, p2) : (id)
-- Bank p1's live synthesized memories from page p2 that, with every memory they replaced down the
-- whole chain, have evidence and all of it from that page (forgetting them must not lose a fact
-- from another source).
WITH RECURSIVE chain(root,id,supersedes_id) AS (
  SELECT m.id,m.id,m.supersedes_id FROM memories m
  WHERE m.bank_id=:p1 AND m.deleted_at IS NULL AND m.origin='synthesized'
    AND EXISTS (SELECT 1 FROM memory_evidence e WHERE e.memory_id=m.id AND e.page_id=:p2)
  UNION ALL
  SELECT c.root,m.id,m.supersedes_id FROM memories m JOIN chain c ON m.id=c.supersedes_id)
SELECT c.root AS id FROM chain c
CROSS JOIN LATERAL (SELECT
  EXISTS (SELECT 1 FROM memory_evidence e WHERE e.memory_id=c.id AND e.page_id=:p2)
  AND NOT EXISTS (SELECT 1 FROM memory_evidence e WHERE e.memory_id=c.id
    AND (e.page_id IS NULL OR e.page_id<>:p2)) AS pure) x
GROUP BY c.root HAVING bool_and(x.pure);
