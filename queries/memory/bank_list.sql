--: Record(agent_id?)

--! run (p1?, p2?) : Record
-- One bank (p1) or all, optionally searched by name (p2), with what the bank page shows; counts
-- are of live, current memories.
SELECT b.id,b.name,b.description,b.scope,b.agent_id,b.created_at,
  COALESCE((SELECT array_agg(l.agent_id ORDER BY l.agent_id) FROM agent_memory_banks l WHERE l.bank_id=b.id), '{}')::UUID[] AS agent_ids,
  (SELECT COUNT(*) FROM memories m WHERE m.bank_id=b.id AND m.deleted_at IS NULL
   AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id)) AS memory_count,
  (SELECT COUNT(*) FROM memory_embedding_jobs j JOIN memories m ON m.id=j.memory_id
   WHERE m.bank_id=b.id AND m.deleted_at IS NULL
   AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id)) AS pending_embeddings
FROM memory_banks b
WHERE (:p1::UUID IS NULL OR b.id=:p1) AND (:p2::TEXT IS NULL OR b.name ILIKE '%'||:p2||'%')
ORDER BY b.name;
