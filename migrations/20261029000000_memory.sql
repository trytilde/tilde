-- Memory: banks of short facts agents recall and write through built-in tools, and people curate.
-- Needs pgvector 0.8 or later (halfvec, iterative HNSW scans).
-- A bank is shared, or private to one agent (deleted with it). Agents reach banks only through
-- `agent_memory_banks`; a per-person bank keeps each verified person's memories apart. Recall
-- fuses full-text search with pgvector similarity over embeddings computed by a background worker
-- through an inference connection (`memory_settings`).
CREATE EXTENSION IF NOT EXISTS vector;
DO $$
BEGIN
 IF (SELECT string_to_array(extversion, '.')::INT[] < ARRAY[0,8] FROM pg_extension WHERE extname='vector') THEN
  RAISE EXCEPTION 'Memory needs the pgvector extension 0.8 or later; update it and run the migration again';
 END IF;
END $$;
CREATE TABLE memory_banks (
 id UUID PRIMARY KEY,
 name TEXT NOT NULL UNIQUE CHECK (name <> '' AND length(name) <= 128),
 description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 2048),
 -- Set for an agent's own bank, which only that agent is linked to.
 agent_id UUID REFERENCES agents(id) ON DELETE CASCADE,
 -- per_person: agents read and write only the memories of the person they act for, plus
 -- bank-wide memories people wrote without one.
 scope TEXT NOT NULL CHECK (scope IN ('shared','per_person')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE agent_memory_banks (
 agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
 bank_id UUID NOT NULL REFERENCES memory_banks(id) ON DELETE CASCADE,
 access TEXT NOT NULL CHECK (access IN ('read','read_write')),
 PRIMARY KEY (agent_id, bank_id)
);
CREATE INDEX agent_memory_banks_by_bank ON agent_memory_banks(bank_id);
CREATE TABLE memories (
 id UUID PRIMARY KEY,
 bank_id UUID NOT NULL REFERENCES memory_banks(id) ON DELETE CASCADE,
 kind TEXT NOT NULL CHECK (kind IN ('profile','preferences','entities','events','cases','patterns','tools','skills')),
 -- Slash-separated segments organising the bank, '' for its root.
 path TEXT NOT NULL CHECK (length(path) <= 512),
 title TEXT NOT NULL CHECK (title <> '' AND length(title) <= 256),
 content TEXT NOT NULL CHECK (content <> '' AND length(content) <= 16384),
 -- When it holds: a point in time is [t,t], a period [start,end), an ongoing one [start,); NULL
 -- when timeless.
 valid_during TSTZRANGE CHECK (
  valid_during IS NULL
  OR (lower(valid_during) = upper(valid_during) AND lower_inc(valid_during) AND upper_inc(valid_during))
  OR (lower(valid_during) < upper(valid_during) AND lower_inc(valid_during) AND NOT upper_inc(valid_during))
  OR (NOT lower_inf(valid_during) AND upper_inf(valid_during) AND lower_inc(valid_during))
 ),
 importance REAL NOT NULL DEFAULT 0.5 CHECK (importance BETWEEN 0 AND 1),
 -- explicit: an agent wrote it. owner: a person wrote or edited it; agents cannot change it.
 origin TEXT NOT NULL CHECK (origin IN ('explicit','owner')),
 -- The person (root identity) it belongs to in a per-person bank.
 identity_id UUID,
 written_by_agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
 -- The memory this one replaces; a superseded memory is kept but no longer recalled. A memory is
 -- superseded at most once.
 supersedes_id UUID REFERENCES memories(id) ON DELETE SET NULL,
 -- Where an agent wrote it: the conversation, and the latest message a person sent there.
 thread_id UUID REFERENCES chat_threads(id),
 message_id UUID REFERENCES chat_messages(id),
  -- Title words rank above content words.
 search TSVECTOR GENERATED ALWAYS AS (
  setweight(to_tsvector('english', title), 'A') || setweight(to_tsvector('english', content), 'B')
 ) STORED,
 -- Untyped so the dimension can follow the embedding model; partial indexes below cover the
 -- supported ones. embedding_version is the memory_settings version it was computed under.
 embedding HALFVEC,
 embedding_dims INTEGER,
 embedding_version INTEGER,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 deleted_at TIMESTAMPTZ
);
CREATE INDEX memories_by_path ON memories(bank_id, path) WHERE deleted_at IS NULL;
CREATE INDEX memories_search ON memories USING GIN(search) WHERE deleted_at IS NULL;
CREATE UNIQUE INDEX memories_superseded ON memories(supersedes_id) WHERE supersedes_id IS NOT NULL;
CREATE INDEX memories_embedding_1024 ON memories USING hnsw ((embedding::halfvec(1024)) halfvec_cosine_ops)
 WHERE embedding_dims = 1024 AND deleted_at IS NULL;
CREATE INDEX memories_embedding_1536 ON memories USING hnsw ((embedding::halfvec(1536)) halfvec_cosine_ops)
 WHERE embedding_dims = 1536 AND deleted_at IS NULL;
-- Every change, append-only: the memory as it was after it. agent_id is NULL when a person made
-- it through management.
CREATE TABLE memory_revisions (
 id BIGSERIAL PRIMARY KEY,
 memory_id UUID NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
 at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 operation TEXT NOT NULL CHECK (operation IN ('create','update','supersede','forget')),
 kind TEXT NOT NULL,
 path TEXT NOT NULL,
 title TEXT NOT NULL,
 content TEXT NOT NULL,
 valid_during TSTZRANGE,
 importance REAL NOT NULL,
 origin TEXT NOT NULL,
 identity_id UUID,
 agent_id UUID REFERENCES agents(id) ON DELETE SET NULL
);
CREATE INDEX memory_revisions_by_memory ON memory_revisions(memory_id, id);
-- The embedding model, through an inference connection. Every change takes a new `version` from
-- the sequence (never reused, even after the settings are cleared), and every memory is embedded
-- again; until then recall uses full text for memories not yet re-embedded.
CREATE SEQUENCE memory_settings_version;
CREATE TABLE memory_settings (
 singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
 connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE RESTRICT,
 model TEXT NOT NULL CHECK (model <> '' AND length(model) <= 256),
 dims INTEGER NOT NULL CHECK (dims IN (1024, 1536)),
 version INTEGER NOT NULL
);
-- Memories waiting for an embedding. A claim pushes run_after out as a lease; requested_at
-- changes when the memory or the settings change again, so a stale result never clears a newer
-- request.
CREATE TABLE memory_embedding_jobs (
 memory_id UUID PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
 requested_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 run_after TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 attempts INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX memory_embedding_jobs_due ON memory_embedding_jobs(run_after);
