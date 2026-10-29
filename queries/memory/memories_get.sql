--: Record(valid_from?, valid_to?, identity_id?, written_by_agent_id?, supersedes_id?, thread_id?, message_id?)

--! run (p1, p2?) : Record
-- Live memories by id with their bank; `embedded` under settings version p2.
SELECT m.id,m.bank_id,b.name AS bank_name,b.scope,m.kind,m.path,m.title,m.content,
  lower(m.valid_during) AS valid_from,upper(m.valid_during) AS valid_to,m.importance,m.origin,
  m.identity_id,m.written_by_agent_id,m.supersedes_id,m.thread_id,m.message_id,
  COALESCE(m.embedding_version=:p2::INTEGER,FALSE) AS embedded,m.created_at,m.updated_at
FROM memories m JOIN memory_banks b ON b.id=m.bank_id
WHERE m.id=ANY(:p1::UUID[]) AND m.deleted_at IS NULL;
