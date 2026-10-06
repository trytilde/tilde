-- Sandboxes: VMs Tilde launches for agents through a sandbox provider connection (E2B or Modal).
-- The VM's image runs `tilde sandbox connect`, which dials back in; the agent then gets the fixed
-- sandbox tools, run by that process, and processes in the VM get the blueprint's own tools and
-- environment through it. A blueprint is the configuration; each sandbox is one VM.
CREATE TABLE sandbox_blueprints (
 id UUID PRIMARY KEY,
 name TEXT NOT NULL UNIQUE CHECK (name <> '' AND length(name) <= 128),
 -- The provider connection sandboxes are launched with; its provider decides how they are
 -- launched, slept and woken. A connection in use by a blueprint cannot be deleted.
 connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE RESTRICT,
 -- The provider's template (E2B) or image (Modal). It must have the tilde CLI on its PATH.
 template TEXT NOT NULL CHECK (template <> '' AND length(template) <= 512),
 -- Which sandbox an invocation uses: one per thread, per agent, per agent and person, one for
 -- every agent using the blueprint, or one per person across those agents. The person-keyed
 -- modes fall back to one per thread when an invocation acts for no verified person.
 reuse TEXT NOT NULL DEFAULT 'thread' CHECK (reuse IN ('thread','agent','agent_identity','global','global_identity')),
 -- A running sandbox sleeps after this long unused, and any is terminated after `terminate_after`.
 -- A launch or wake waits `connect_timeout` for the sandbox's process to connect.
 sleep_after_secs INTEGER NOT NULL DEFAULT 600 CHECK (sleep_after_secs BETWEEN 60 AND 86400),
 terminate_after_secs INTEGER NOT NULL DEFAULT 604800 CHECK (terminate_after_secs BETWEEN 3600 AND 2592000),
 connect_timeout_secs INTEGER NOT NULL DEFAULT 120 CHECK (connect_timeout_secs BETWEEN 10 AND 600),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 CHECK (terminate_after_secs >= sleep_after_secs)
);
-- Environment variables set in every shell of the blueprint's sandboxes, each value sealed.
-- `TILDE_` names belong to the sandbox process.
CREATE TABLE sandbox_blueprint_env (
 blueprint_id UUID NOT NULL REFERENCES sandbox_blueprints(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK (name ~ '^[A-Za-z_][A-Za-z0-9_]{0,127}$' AND upper(name) NOT LIKE 'TILDE\_%'),
 encrypted_value BYTEA NOT NULL,
 PRIMARY KEY(blueprint_id, name)
);
-- An agent's sandbox setting: every invocation of the agent runs with a sandbox of the blueprint.
-- An agent has at most one. A blueprint in use by an agent cannot be deleted.
CREATE TABLE agent_sandboxes (
 agent_id UUID PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
 blueprint_id UUID NOT NULL REFERENCES sandbox_blueprints(id) ON DELETE RESTRICT
);
CREATE INDEX agent_sandboxes_by_blueprint ON agent_sandboxes(blueprint_id);
-- One VM. Its key is the blueprint and reuse mode it was launched for, plus the agent, thread and
-- person (root identity) that mode distinguishes. Agent and thread are plain IDs: a VM must be
-- terminated at its provider before its row goes, so rows outliving their agent, thread or
-- blueprint setting are found and terminated by the sandbox sweeper rather than cascaded away.
CREATE TABLE sandboxes (
 id UUID PRIMARY KEY,
 blueprint_id UUID NOT NULL REFERENCES sandbox_blueprints(id) ON DELETE RESTRICT,
 reuse TEXT NOT NULL CHECK (reuse IN ('thread','agent','agent_identity','global','global_identity')),
 -- The connection it was launched with, which sleeps, wakes and terminates it: a blueprint moved
 -- to another connection (account) leaves its sandboxes on the old one until they are terminated.
 connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE RESTRICT,
 agent_id UUID,
 thread_id UUID,
 identity_id UUID,
 status TEXT NOT NULL DEFAULT 'starting' CHECK (status IN ('starting','running','sleeping','failed')),
 -- The provider's ID for the VM; for Modal, a sleeping sandbox's filesystem snapshot (an image),
 -- which the next wake launches a new VM from.
 provider_sandbox_id TEXT,
 snapshot_id TEXT,
 error TEXT NOT NULL DEFAULT '' CHECK (length(error) <= 2048),
 -- A single-use enrollment token, exchanged by the sandbox process's first Connect for the
 -- session token that authenticates it afterwards. Digests only.
 enrollment_hash BYTEA UNIQUE,
 session_hash BYTEA UNIQUE,
 -- Refreshed while the sandbox process's Connect stream is open; liveness is judged against it.
 connected_at TIMESTAMPTZ,
 -- Held by the gateway process launching, waking, sleeping or terminating the VM. Another
 -- process may take over once it lapses.
 lease_until TIMESTAMPTZ,
 -- When the provider pauses (E2B, at its TTL) or ends (Modal, at its lifetime) the VM by itself.
 -- The sweeper renews an E2B VM's TTL in time and puts a Modal VM to sleep before its end.
 expires_at TIMESTAMPTZ,
 last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 -- The latest invocation that used it: calls its processes make outside any operation (from a
 -- background job, say) are traced as part of it.
 invocation_id UUID,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 CONSTRAINT sandbox_key CHECK (
  (reuse = 'thread' AND agent_id IS NOT NULL AND thread_id IS NOT NULL AND identity_id IS NULL) OR
  (reuse = 'agent' AND agent_id IS NOT NULL AND thread_id IS NULL AND identity_id IS NULL) OR
  (reuse = 'agent_identity' AND agent_id IS NOT NULL AND num_nonnulls(thread_id, identity_id) = 1) OR
  (reuse = 'global' AND agent_id IS NULL AND thread_id IS NULL AND identity_id IS NULL) OR
  (reuse = 'global_identity' AND ((identity_id IS NOT NULL AND agent_id IS NULL AND thread_id IS NULL) OR
   (identity_id IS NULL AND agent_id IS NOT NULL AND thread_id IS NOT NULL)))
 )
);
CREATE UNIQUE INDEX sandboxes_key ON sandboxes(blueprint_id, reuse, agent_id, thread_id, identity_id) NULLS NOT DISTINCT;
CREATE INDEX sandboxes_by_agent ON sandboxes(agent_id);
-- A transient queue between the gateway process serving an agent's sandbox tool call and
-- whichever process holds the sandbox's Connect stream, as tool_host_calls is for tool hosts.
-- chat_tool_calls stays the record. One agent tool call may queue several operations
-- (apply_patch reads and writes each file), so rows have their own IDs. Each carries the agent
-- tool call's invocation and trace context: tools a process calls while running it are traced
-- inside that call.
CREATE TABLE sandbox_calls (
 id UUID PRIMARY KEY,
 sandbox_id UUID NOT NULL REFERENCES sandboxes(id) ON DELETE CASCADE,
 operation TEXT NOT NULL,
 input_json TEXT NOT NULL,
 invocation_id UUID NOT NULL,
 traceparent TEXT NOT NULL DEFAULT '',
 tracestate TEXT NOT NULL DEFAULT '',
 status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','delivered','completed','failed')),
 output_json TEXT NOT NULL DEFAULT '',
 error TEXT NOT NULL DEFAULT '',
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX sandbox_calls_pending ON sandbox_calls(sandbox_id) WHERE status = 'pending';
CREATE TRIGGER sandbox_calls_notify AFTER INSERT OR UPDATE ON sandbox_calls
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sandbox_calls');

-- Tool sources are owned by an agent or by a sandbox blueprint: a blueprint's tools are offered to
-- processes inside its sandboxes. An agent with a sandbox also has the sandbox itself as a source
-- of the fixed sandbox tools; that source goes with the agent's sandbox setting.
ALTER TABLE agent_tool_sources ALTER COLUMN agent_id DROP NOT NULL;
ALTER TABLE agent_tool_sources ADD COLUMN sandbox_blueprint_id UUID REFERENCES sandbox_blueprints(id) ON DELETE CASCADE;
ALTER TABLE agent_tool_sources ADD COLUMN sandbox_agent_id UUID REFERENCES agent_sandboxes(agent_id) ON DELETE CASCADE;
ALTER TABLE agent_tool_sources DROP CONSTRAINT agent_tool_sources_check;
ALTER TABLE agent_tool_sources ADD CONSTRAINT tool_source_owner CHECK (num_nonnulls(agent_id, sandbox_blueprint_id) = 1);
ALTER TABLE agent_tool_sources ADD CONSTRAINT tool_source_target CHECK (
 num_nonnulls(connection_id, tool_host_id, sandbox_agent_id) = 1 AND (sandbox_agent_id IS NULL OR sandbox_agent_id = agent_id)
);
ALTER TABLE agent_tool_sources ADD CONSTRAINT tool_source_blueprint_slug UNIQUE(sandbox_blueprint_id, slug);
ALTER TABLE agent_tool_sources ADD CONSTRAINT tool_source_blueprint_connection UNIQUE(sandbox_blueprint_id, connection_id);
ALTER TABLE agent_tool_sources ADD CONSTRAINT tool_source_blueprint_tool_host UNIQUE(sandbox_blueprint_id, tool_host_id);
ALTER TABLE agent_tool_sources ADD CONSTRAINT tool_source_sandbox UNIQUE(sandbox_agent_id);

-- Processes inside a sandbox call tool hosts too. The call names the sandbox; a sandbox shared
-- across threads or agents has no thread or agent of its own to name.
ALTER TABLE tool_host_calls ADD COLUMN sandbox_id UUID;
ALTER TABLE tool_host_calls DROP CONSTRAINT tool_host_call_shape;
ALTER TABLE tool_host_calls ADD CONSTRAINT tool_host_call_shape CHECK (
 (kind = 'call' AND ((agent_id IS NOT NULL AND thread_id IS NOT NULL) OR sandbox_id IS NOT NULL)) OR
 (kind = 'verify' AND connection_id IS NOT NULL AND credentials IS NOT NULL)
);
