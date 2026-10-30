--! run (p1, p2) : (only)
-- Whether memory p1 and every memory it replaced, down the whole chain, have evidence and all of
-- it from page p2: forgetting it then loses no fact from another source.
WITH RECURSIVE chain(id,supersedes_id) AS (
  SELECT id,supersedes_id FROM memories WHERE id=:p1
  UNION ALL
  SELECT m.id,m.supersedes_id FROM memories m JOIN chain c ON m.id=c.supersedes_id)
SELECT EXISTS (SELECT 1 FROM chain) AND NOT EXISTS (SELECT 1 FROM chain c
  WHERE NOT EXISTS (SELECT 1 FROM memory_evidence e WHERE e.memory_id=c.id AND e.page_id=:p2)
    OR EXISTS (SELECT 1 FROM memory_evidence e WHERE e.memory_id=c.id
      AND (e.page_id IS NULL OR e.page_id<>:p2))) AS only;
