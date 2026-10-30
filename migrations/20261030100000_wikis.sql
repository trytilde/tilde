-- Wikis: trees of Markdown pages that people write and agents read and edit through built-in
-- tools. A wiki is shared, or an agent's own (deleted with it). Agents reach wikis only through
-- `agent_wikis`. Pages are found by path, by ranked full text and by grep; every change is a
-- revision, and a write names the revision it read so concurrent edits never overwrite each other.
CREATE TABLE wikis (
 id UUID PRIMARY KEY,
 name TEXT NOT NULL UNIQUE CHECK (name <> '' AND length(name) <= 128),
 description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 2048),
 -- Set for an agent's own wiki, which only that agent is linked to.
 agent_id UUID REFERENCES agents(id) ON DELETE CASCADE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- An agent has at most one wiki of its own.
CREATE UNIQUE INDEX wikis_own ON wikis(agent_id) WHERE agent_id IS NOT NULL;
CREATE TABLE agent_wikis (
 agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
 wiki_id UUID NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
 access TEXT NOT NULL CHECK (access IN ('read','read_write')),
 PRIMARY KEY (agent_id, wiki_id)
);
CREATE INDEX agent_wikis_by_wiki ON agent_wikis(wiki_id);
CREATE TABLE wiki_pages (
 id UUID PRIMARY KEY,
 wiki_id UUID NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
 -- Slash-separated segments, unique in the wiki.
 path TEXT NOT NULL CHECK (path <> '' AND length(path) <= 512),
 title TEXT NOT NULL CHECK (title <> '' AND length(title) <= 256),
 content TEXT NOT NULL CHECK (length(content) <= 262144),
 -- The latest revision's number; a write must name it.
 revision INTEGER NOT NULL,
 -- Title words rank above content words.
 search TSVECTOR GENERATED ALWAYS AS (
  setweight(to_tsvector('english', title), 'A') || setweight(to_tsvector('english', content), 'B')
 ) STORED,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 UNIQUE (wiki_id, path)
);
CREATE INDEX wiki_pages_search ON wiki_pages USING GIN(search);
-- Every change, append-only: the page as it was after it (a delete keeps the last content).
-- Deleting a page removes its row and keeps its history; agent_id is NULL when a person made it.
CREATE TABLE wiki_page_revisions (
 page_id UUID NOT NULL,
 wiki_id UUID NOT NULL REFERENCES wikis(id) ON DELETE CASCADE,
 revision INTEGER NOT NULL,
 operation TEXT NOT NULL CHECK (operation IN ('create','update','move','delete')),
 path TEXT NOT NULL,
 title TEXT NOT NULL,
 content TEXT NOT NULL,
 agent_id UUID REFERENCES agents(id) ON DELETE SET NULL,
 -- The conversation an agent wrote it in; unset for a person, or for a sidecar conversation the
 -- gateway has not projected yet.
 thread_id UUID REFERENCES chat_threads(id),
 -- The write that made it: an agent's tool call id, or a fresh id for a person's change. A
 -- retried call finds its revision here and gets the same result instead of a second change.
 call_id UUID NOT NULL UNIQUE,
 at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY (page_id, revision)
);
CREATE INDEX wiki_page_revisions_by_wiki ON wiki_page_revisions(wiki_id);
