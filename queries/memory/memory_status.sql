--: Record(identity_id?, written_by_agent_id?, supersedes_id?, forgotten_by?)

--! run (p1) : Record
-- What an agent's change needs to know of a memory, forgotten ones included.
SELECT m.bank_id,m.origin,m.identity_id,m.written_by_agent_id,m.supersedes_id,m.deleted_at IS NOT NULL AS deleted,
  EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id) AS superseded,
  (SELECT r.agent_id FROM memory_revisions r WHERE r.memory_id=m.id AND r.operation='forget' ORDER BY r.id DESC LIMIT 1) AS forgotten_by
FROM memories m WHERE m.id=:p1;
