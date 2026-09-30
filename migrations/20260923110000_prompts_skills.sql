-- Prompts are declared in agent code and registered with a deployment by `tilde deploy`. The
-- platform never edits them: every distinct (template, sections, config) becomes an immutable
-- version, identified by its content hash, so inference calls can be compared across changes.
CREATE TABLE prompts (
 id UUID PRIMARY KEY,
 agent_id UUID NOT NULL REFERENCES agents(id),
 name TEXT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 UNIQUE (agent_id, name)
);
CREATE TABLE prompt_versions (
 id UUID PRIMARY KEY,
 prompt_id UUID NOT NULL REFERENCES prompts(id) ON DELETE CASCADE,
 number INTEGER NOT NULL,
 -- SHA-256 over the template, its sections and config; the SDK computes the same hash.
 hash BYTEA NOT NULL,
 template TEXT NOT NULL,
 -- Model settings the agent versions with the text (JSON object as the SDK serialised it).
 config TEXT NOT NULL,
 -- Placeholders found in the template and its sections (`{{name}}` or `{name}` by format).
 variables TEXT[] NOT NULL,
 -- plain: no variables; mustache: `{{var}}` and `{{> section}}`; braces: `{var}`; dynamic: a
 -- function whose source is the template, reached by the SDK's stamp rather than by text.
 format TEXT NOT NULL CHECK (format IN ('plain','mustache','braces','dynamic')),
 -- Where the SDK found it in the agent's code, e.g. `config/agents.yaml#researcher.goal`.
 origin TEXT NOT NULL,
 -- The first deployment that shipped this version.
 deployment_id UUID REFERENCES agent_deployments(id),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 UNIQUE (prompt_id, hash),
 UNIQUE (prompt_id, number)
);
-- Reusable partials referenced from the template as `{{> name}}`, kept per version so a
-- shared section's change is visible on each prompt that uses it.
CREATE TABLE prompt_sections (
 version_id UUID NOT NULL REFERENCES prompt_versions(id) ON DELETE CASCADE,
 name TEXT NOT NULL,
 hash BYTEA NOT NULL,
 content TEXT NOT NULL,
 PRIMARY KEY (version_id, name)
);
-- Every prompt version a deployment shipped; a version may ship with many deployments.
CREATE TABLE deployment_prompts (
 deployment_id UUID NOT NULL REFERENCES agent_deployments(id) ON DELETE CASCADE,
 version_id UUID NOT NULL REFERENCES prompt_versions(id) ON DELETE CASCADE,
 PRIMARY KEY (deployment_id, version_id)
);
CREATE INDEX deployment_prompts_version ON deployment_prompts(version_id);
-- The prompt versions an inference call used: static text the gateway found in the request
-- body, or `x-tilde-prompt: name@hash` stamps the SDK sent for dynamic prompts. Forgetting a
-- prompt's history drops its links; the request rows stay.
CREATE TABLE inference_request_prompts (
 request_id UUID NOT NULL REFERENCES inference_requests(id) ON DELETE CASCADE,
 version_id UUID NOT NULL REFERENCES prompt_versions(id) ON DELETE CASCADE,
 PRIMARY KEY (request_id, version_id)
);
CREATE INDEX inference_request_prompts_version ON inference_request_prompts(version_id);

-- Skill sources group skills: a Tilde catalog group compiled into the engine,
-- a git repository synced through the GitHub API, or an editor collection authored in the UI.
-- Agents are assigned whole sources (and so every skill a sync adds) or single skills. A code
-- source holds the skills an agent's own deployments ship: one per agent, created by its first
-- deployment with skills, governed by its agent and never listed or assigned.
CREATE TABLE skill_sources (
 id UUID PRIMARY KEY,
 slug TEXT NOT NULL UNIQUE CHECK (slug ~ '^[a-z0-9][a-z0-9-]{0,62}$'),
 name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
 kind TEXT NOT NULL CHECK (kind IN ('catalog','git','editor','bundled')),
 catalog_group TEXT UNIQUE,
 agent_id UUID UNIQUE REFERENCES agents(id),
 repository_url TEXT,
 git_ref TEXT,
 -- Repository subdirectory scanned for SKILL.md files; empty scans the whole tree.
 git_path TEXT NOT NULL DEFAULT '',
 -- The commit the current skills came from, and the outcome of the last sync attempt.
 commit_sha TEXT,
 synced_at TIMESTAMPTZ,
 sync_error TEXT,
 -- Bumped when a sync starts; a sync whose fetch finishes after a newer one started is dropped.
 sync_generation BIGINT NOT NULL DEFAULT 0,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 CHECK ((kind='catalog') = (catalog_group IS NOT NULL)),
 CHECK ((kind='git') = (repository_url IS NOT NULL AND git_ref IS NOT NULL)),
 CHECK ((kind='bundled') = (agent_id IS NOT NULL))
);
CREATE TABLE skills (
 id UUID PRIMARY KEY,
 source_id UUID NOT NULL REFERENCES skill_sources(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK (name ~ '^[a-z0-9][a-z0-9_-]{0,63}$'),
 -- Directory of the SKILL.md within the source (repository path for git sources).
 source_path TEXT NOT NULL DEFAULT '',
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 UNIQUE (source_id, name)
);
CREATE TABLE skill_versions (
 id UUID PRIMARY KEY,
 skill_id UUID NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
 number INTEGER NOT NULL,
 -- SHA-256 over each file's path, content digest and executable bit, in path order.
 hash BYTEA NOT NULL,
 -- From the SKILL.md front matter; what an agent sees before reading the skill.
 description TEXT NOT NULL,
 message TEXT NOT NULL,
 commit_sha TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 UNIQUE (skill_id, number)
);
-- Every file is a content-addressed object in the skills bucket; UTF-8 text up to 256 KiB is
-- also kept inline so the UI and agents read it without a round trip to object storage.
CREATE TABLE skill_files (
 version_id UUID NOT NULL REFERENCES skill_versions(id) ON DELETE CASCADE,
 path TEXT NOT NULL,
 media_type TEXT NOT NULL,
 size_bytes BIGINT NOT NULL,
 sha256 BYTEA NOT NULL,
 executable BOOLEAN NOT NULL DEFAULT false,
 content TEXT,
 object_key TEXT,
 PRIMARY KEY (version_id, path),
 CHECK (content IS NOT NULL OR object_key IS NOT NULL)
);
-- The skill versions a deployment shipped from the agent's bundled source. Bundled skills may have
-- several live versions at once, one per deployment still running.
CREATE TABLE deployment_skills (
 deployment_id UUID NOT NULL REFERENCES agent_deployments(id) ON DELETE CASCADE,
 version_id UUID NOT NULL REFERENCES skill_versions(id) ON DELETE CASCADE,
 origin TEXT NOT NULL,
 PRIMARY KEY (deployment_id, version_id)
);
CREATE INDEX deployment_skills_version ON deployment_skills(version_id);
CREATE TABLE agent_skill_sources (
 agent_id UUID NOT NULL REFERENCES agents(id),
 source_id UUID NOT NULL REFERENCES skill_sources(id) ON DELETE CASCADE,
 -- A group is added switched off. Enabled gives the agent every skill in it (those a later sync
 -- adds too) except the ones in agent_skill_exclusions; disabled gives none of them.
 enabled BOOLEAN NOT NULL DEFAULT FALSE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY (agent_id, source_id)
);
CREATE TABLE agent_skills (
 agent_id UUID NOT NULL REFERENCES agents(id),
 skill_id UUID NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY (agent_id, skill_id)
);
-- Skills of an enabled group that the agent switched off; later syncs still add new skills on.
CREATE TABLE agent_skill_exclusions (
 agent_id UUID NOT NULL REFERENCES agents(id),
 skill_id UUID NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
 PRIMARY KEY (agent_id, skill_id)
);
-- Connections carry skills too: skills linked to a connection reach every agent assigned the
-- connection's skills capability, so an agent given a Slack account also learns to use it.
-- Every connection offers the capability; its type decides only chat and inference.
ALTER TABLE connection_agents DROP CONSTRAINT connection_agents_capability_check;
ALTER TABLE connection_agents ADD CONSTRAINT connection_agents_capability_check CHECK (capability IN ('channel','inference','skills'));
CREATE TABLE connection_skill_sources (
 connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
 source_id UUID NOT NULL REFERENCES skill_sources(id) ON DELETE CASCADE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY (connection_id, source_id)
);
CREATE TABLE connection_skills (
 connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
 skill_id UUID NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY (connection_id, skill_id)
);
CREATE INDEX agent_skill_sources_source ON agent_skill_sources(source_id);
CREATE INDEX agent_skills_skill ON agent_skills(skill_id);

