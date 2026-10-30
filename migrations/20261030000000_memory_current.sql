-- What recall, browse and the bank page see: live memories no other memory replaced, with their
-- bank's scope. Each query adds its own person filter, which takes parameters.
CREATE VIEW current_memories AS
SELECT m.id, m.bank_id, m.kind, m.path, m.title, m.content, m.valid_during, m.importance,
 m.origin, m.identity_id, m.written_by_agent_id, m.supersedes_id, m.thread_id, m.message_id,
 m.search, m.embedding, m.embedding_dims, m.embedding_version, m.created_at, m.updated_at,
 b.scope
FROM memories m JOIN memory_banks b ON b.id=m.bank_id
WHERE m.deleted_at IS NULL AND NOT EXISTS (SELECT 1 FROM memories s WHERE s.supersedes_id=m.id);
-- The bank page lists a bank's memories newest first, a page at a time.
CREATE INDEX memories_newest ON memories(bank_id, created_at DESC, id DESC) WHERE deleted_at IS NULL;
-- An agent has at most one bank of its own.
CREATE UNIQUE INDEX memory_banks_own ON memory_banks(agent_id) WHERE agent_id IS NOT NULL;
