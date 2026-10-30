-- Memory synthesis: Tilde turns a bank's sources (conversations of agents linked with
-- `auto_retain`, and pages of wikis linked to the bank) into memories through the bank's own chat
-- model. Each batch is one source's unprocessed part; its id is derived from that part, and it is
-- applied, with the source's watermark, in one transaction, so a retry never applies it twice.
ALTER TABLE memories DROP CONSTRAINT memories_origin_check;
-- synthesized: Tilde derived it from the bank's sources. Agents and people may change it; Tilde
-- never changes an owner memory.
ALTER TABLE memories ADD CONSTRAINT memories_origin_check CHECK (origin IN ('explicit','owner','synthesized'));
-- Who made a change: a person (through management), an agent (agent_id) or the synthesizer.
ALTER TABLE memory_revisions ADD COLUMN actor TEXT;
-- An agent's create or supersede of an agent-written memory stays the agent's even when the agent
-- row is gone; other changes without an agent were a person's.
UPDATE memory_revisions SET actor = CASE
  WHEN agent_id IS NOT NULL OR (origin = 'explicit' AND operation IN ('create','supersede')) THEN 'agent'
  ELSE 'person' END;
ALTER TABLE memory_revisions ALTER COLUMN actor SET NOT NULL;
-- agent_id is NULL for people and the synthesizer, and for an agent whose row was since removed.
ALTER TABLE memory_revisions ADD CONSTRAINT memory_revisions_actor_check
 CHECK (actor IN ('person','agent','synthesizer') AND (agent_id IS NULL OR actor = 'agent'));
-- The synthesis batch that made a synthesizer's change.
ALTER TABLE memory_revisions ADD COLUMN batch_id UUID CHECK (batch_id IS NULL OR actor = 'synthesizer');
CREATE INDEX memory_revisions_by_batch ON memory_revisions(batch_id) WHERE batch_id IS NOT NULL;
-- What a synthesized memory was derived from, and in which batch: messages of a conversation,
-- or a wiki page revision. Memories agents write keep their conversation on `memories` itself.
CREATE TABLE memory_evidence (
 id BIGSERIAL PRIMARY KEY,
 memory_id UUID NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
 batch_id UUID NOT NULL,
 thread_id UUID REFERENCES chat_threads(id),
 message_id UUID REFERENCES chat_messages(id),
 -- The page is kept by id: its revisions outlive it.
 page_id UUID,
 revision INTEGER,
 CHECK ((message_id IS NOT NULL AND thread_id IS NOT NULL AND page_id IS NULL AND revision IS NULL)
     OR (page_id IS NOT NULL AND revision IS NOT NULL AND thread_id IS NULL AND message_id IS NULL))
);
CREATE INDEX memory_evidence_by_memory ON memory_evidence(memory_id);
-- A page batch shows the model the memories derived from that page; a batch lists what it wrote.
CREATE INDEX memory_evidence_by_page ON memory_evidence(page_id) WHERE page_id IS NOT NULL;
CREATE INDEX memory_evidence_by_batch ON memory_evidence(batch_id);
-- A bank's synthesis, on while the row exists: its chat model and the state of its worker. Saving
-- the model probes it first. `run_after` backs the bank off after a failed batch; `lease_until`
-- holds a claim. Turning synthesis off deletes the row and keeps the watermarks.
CREATE TABLE memory_synthesis (
 bank_id UUID PRIMARY KEY REFERENCES memory_banks(id) ON DELETE CASCADE,
 connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE RESTRICT,
 model TEXT NOT NULL CHECK (model <> '' AND length(model) <= 256),
 -- A short hint for the model from whoever manages the bank.
 instructions TEXT NOT NULL DEFAULT '' CHECK (length(instructions) <= 2000),
 run_after TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 lease_until TIMESTAMPTZ NOT NULL DEFAULT '-infinity',
 attempts INTEGER NOT NULL DEFAULT 0,
 last_run_at TIMESTAMPTZ,
 last_error TEXT NOT NULL DEFAULT '' CHECK (length(last_error) <= 2000)
);
-- An agent's conversations feed every bank it is linked to read-write with auto_retain.
ALTER TABLE agent_memory_banks ADD COLUMN auto_retain BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE agent_memory_banks ADD CONSTRAINT agent_memory_banks_auto_retain_check
 CHECK (NOT auto_retain OR access = 'read_write');
-- A wiki whose pages feed the bank.
CREATE TABLE memory_bank_wikis (
 bank_id UUID NOT NULL REFERENCES memory_banks(id) ON DELETE CASCADE,
 wiki_id UUID NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
 PRIMARY KEY (bank_id, wiki_id)
);
CREATE INDEX memory_bank_wikis_by_wiki ON memory_bank_wikis(wiki_id);
-- How far each source has been synthesized into each bank. A conversation by its messages'
-- (created_at, id); a page by its revision number, which only grows under the page's row lock. A
-- page's row outlives the page, so its deletion is noticed.
CREATE TABLE memory_synthesis_threads (
 bank_id UUID NOT NULL REFERENCES memory_banks(id) ON DELETE CASCADE,
 thread_id UUID NOT NULL REFERENCES chat_threads(id),
 through_at TIMESTAMPTZ NOT NULL,
 through_id UUID NOT NULL,
 PRIMARY KEY (bank_id, thread_id)
);
CREATE TABLE memory_synthesis_pages (
 bank_id UUID NOT NULL REFERENCES memory_banks(id) ON DELETE CASCADE,
 page_id UUID NOT NULL,
 revision INTEGER NOT NULL,
 PRIMARY KEY (bank_id, page_id)
);
-- Each batch: applied once (with what it wrote, replaced, forgot and dropped), failed with why, or
-- skipped. `start_id` names where its source part starts (bank, source, watermark): failures the
-- content caused (`failures`) are counted across the batches from one start, and a start that
-- keeps failing is passed over.
CREATE TABLE memory_synthesis_batches (
 id UUID PRIMARY KEY,
 bank_id UUID NOT NULL REFERENCES memory_banks(id) ON DELETE CASCADE,
 start_id UUID NOT NULL,
 thread_id UUID REFERENCES chat_threads(id),
 page_id UUID,
 revision INTEGER,
 status TEXT NOT NULL CHECK (status IN ('applied','failed','skipped')),
 failures INTEGER NOT NULL DEFAULT 0,
 written INTEGER NOT NULL DEFAULT 0,
 superseded INTEGER NOT NULL DEFAULT 0,
 forgotten INTEGER NOT NULL DEFAULT 0,
 dropped INTEGER NOT NULL DEFAULT 0,
 -- Why operations were dropped, or why the batch failed.
 notes TEXT NOT NULL DEFAULT '' CHECK (length(notes) <= 4000),
 at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 CHECK ((thread_id IS NULL) <> (page_id IS NULL))
);
CREATE INDEX memory_synthesis_batches_recent ON memory_synthesis_batches(bank_id, at DESC);
CREATE INDEX memory_synthesis_batches_by_start ON memory_synthesis_batches(start_id);
