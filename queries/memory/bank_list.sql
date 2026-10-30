--: Record(agent_id?)

--! run (p1?, p2?) : Record
-- One bank (p1) or all, optionally searched by name (p2), with what the bank page shows; counts
-- are of live, current memories.
SELECT b.id,b.name,b.description,b.scope,b.agent_id,b.created_at,
  -- The linked agents and, in the same order, each one's access.
  COALESCE((SELECT array_agg(l.agent_id ORDER BY l.agent_id) FROM agent_memory_banks l WHERE l.bank_id=b.id), '{}')::UUID[] AS agent_ids,
  COALESCE((SELECT array_agg(l.access ORDER BY l.agent_id) FROM agent_memory_banks l WHERE l.bank_id=b.id), '{}')::TEXT[] AS link_access,
  (SELECT COUNT(*) FROM current_memories m WHERE m.bank_id=b.id) AS memory_count,
  -- Without embedding settings nothing is waiting: jobs only run once a model is set.
  (SELECT COUNT(*) FROM memory_embedding_jobs j JOIN current_memories m ON m.id=j.memory_id
   WHERE m.bank_id=b.id AND EXISTS (SELECT 1 FROM memory_settings)) AS pending_embeddings
FROM memory_banks b
WHERE (:p1::UUID IS NULL OR b.id=:p1) AND (:p2::TEXT IS NULL OR b.name ILIKE '%'||:p2||'%')
ORDER BY b.name;
