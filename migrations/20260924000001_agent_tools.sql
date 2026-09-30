-- Agents' tools. Tools are a third connection capability: unlike channel and inference they are not
-- granted through connection_agents but by an agent adding the connection (or a tool host) as one
-- of its tool sources; tools_invoke still filters the resulting catalog names.
ALTER TABLE connection_types ADD COLUMN tool_capable BOOLEAN NOT NULL DEFAULT false;
-- A connection type may serve its tools from the MCP server at `mcp_url`; `mcp_credential` says how
-- the connection's credential reaches it. Both NULL means the type's tools are native or it has none.
ALTER TABLE connection_types ADD COLUMN mcp_url TEXT;
ALTER TABLE connection_types ADD COLUMN mcp_credential TEXT;
ALTER TABLE connection_types ADD COLUMN mcp_credential_name TEXT;
ALTER TABLE connection_types ADD COLUMN mcp_credential_prefix TEXT NOT NULL DEFAULT '';
ALTER TABLE connection_types ADD CONSTRAINT connection_type_mcp CHECK (
 (mcp_credential IS NULL) = (mcp_url IS NULL)
 AND (mcp_credential IS NULL OR mcp_credential IN ('none','bearer','header','query'))
 AND ((mcp_credential IN ('header','query')) = (mcp_credential_name IS NOT NULL))
);
-- Where an OAuth type's client comes from: the setup form, or dynamic client registration
-- against the MCP server at setup.
ALTER TABLE connection_types ADD COLUMN oauth_client TEXT NOT NULL DEFAULT 'form' CHECK (oauth_client IN ('form','dynamic'));

-- A personal connection belongs to the chat user who set it up and is offered only to agents
-- conversing with that user (or another identity under the same root). Installation-owned
-- connections keep a NULL owner. A personal connection is never an agent's tool source: that
-- would hand one person's account to every user of an agent.
ALTER TABLE connections ADD COLUMN owner_user_id UUID REFERENCES chat_users(id) ON DELETE CASCADE;
CREATE INDEX connections_by_owner ON connections(owner_user_id) WHERE owner_user_id IS NOT NULL;

-- Customer-hosted tool backends. A connected host authenticates Watch with a token whose digest
-- is stored here; a Lambda host is its function ARN. Hosts are never dialed.
CREATE TABLE tool_hosts (
 id UUID PRIMARY KEY,
 name TEXT NOT NULL UNIQUE CHECK (name <> '' AND length(name) <= 128),
 execution_type TEXT NOT NULL CHECK (execution_type IN ('connected','lambda')),
 function_arn TEXT,
 token_hash BYTEA UNIQUE,
 -- Refreshed while a Watch stream is open; liveness is judged against it.
 connected_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 CHECK ((execution_type = 'lambda') = (function_arn IS NOT NULL)),
 CHECK (execution_type = 'connected' OR token_hash IS NULL)
);
-- The host's own description of its tools: replaced on every Watch or Lambda refresh.
CREATE TABLE tool_host_tools (
 tool_host_id UUID NOT NULL REFERENCES tool_hosts(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK (name <> '' AND length(name) <= 128),
 description TEXT NOT NULL CHECK (description <> ''),
 summary TEXT NOT NULL DEFAULT '',
 input_schema_json TEXT NOT NULL,
 output_schema_json TEXT NOT NULL DEFAULT '',
 read_only BOOLEAN NOT NULL DEFAULT false,
 destructive BOOLEAN NOT NULL DEFAULT false,
 idempotent BOOLEAN NOT NULL DEFAULT false,
 open_world BOOLEAN NOT NULL DEFAULT false,
 PRIMARY KEY(tool_host_id, name)
);
-- A tool host may publish a provider definition; each connection of that provider is an instance
-- of the host whose credentials Tilde stores and sends with every call. The provider belongs to
-- the host: only it can replace the definition, and deleting the host removes it.
ALTER TABLE connection_providers ADD COLUMN tool_host_id UUID UNIQUE REFERENCES tool_hosts(id) ON DELETE CASCADE;
-- A transient queue between the gateway process serving InvokeTool and whichever process holds
-- the host's Watch stream: a tool call, or the verification of an instance's credentials at the
-- end of its setup. chat_tool_calls stays the durable record; rows here are deleted once their
-- result is read and expire otherwise. A call's ID is the agent's tool call ID. Credentials
-- travel sealed with the row's ID and are deleted with it; a verify has no agent or thread.
CREATE TABLE tool_host_calls (
 id UUID PRIMARY KEY,
 tool_host_id UUID NOT NULL REFERENCES tool_hosts(id) ON DELETE CASCADE,
 name TEXT NOT NULL,
 input_json TEXT NOT NULL,
 agent_id UUID,
 thread_id UUID,
 status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','delivered','completed','failed')),
 output_json TEXT NOT NULL DEFAULT '',
 error TEXT NOT NULL DEFAULT '',
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 kind TEXT NOT NULL DEFAULT 'call' CHECK (kind IN ('call','verify')),
 connection_id UUID REFERENCES connections(id) ON DELETE CASCADE,
 credentials BYTEA,
 CONSTRAINT tool_host_call_shape CHECK (
  (kind = 'call' AND agent_id IS NOT NULL AND thread_id IS NOT NULL) OR
  (kind = 'verify' AND connection_id IS NOT NULL AND credentials IS NOT NULL)
 ),
 CONSTRAINT tool_host_call_credentials CHECK ((connection_id IS NULL) = (credentials IS NULL))
);
CREATE INDEX tool_host_calls_pending ON tool_host_calls(tool_host_id) WHERE status = 'pending';
CREATE TRIGGER tool_host_calls_notify AFTER INSERT OR UPDATE ON tool_host_calls
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_tool_host_calls');
-- Availability samples for tool hosts, taken by the agent health sweep for the 12-hour history.
-- A connected host is healthy while its Watch stream is live; a Lambda host is always available,
-- as a Lambda agent deployment is.
CREATE TABLE tool_host_health (
 id BIGSERIAL PRIMARY KEY,
 tool_host_id UUID NOT NULL REFERENCES tool_hosts(id) ON DELETE CASCADE,
 checked_at TIMESTAMPTZ NOT NULL,
 healthy BOOLEAN NOT NULL
);
CREATE INDEX tool_host_health_history ON tool_host_health(tool_host_id, checked_at);
CREATE INDEX tool_host_health_retention ON tool_host_health(checked_at);

-- An MCP-served connection's tools, discovered from its server; the hash makes an unchanged
-- rediscovery a no-op.
CREATE TABLE connection_tools (
 connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK (name <> '' AND length(name) <= 128),
 description TEXT NOT NULL CHECK (description <> ''),
 input_schema_json TEXT NOT NULL,
 output_schema_json TEXT NOT NULL DEFAULT '',
 read_only BOOLEAN NOT NULL DEFAULT false,
 destructive BOOLEAN NOT NULL DEFAULT false,
 idempotent BOOLEAN NOT NULL DEFAULT false,
 open_world BOOLEAN NOT NULL DEFAULT false,
 PRIMARY KEY(connection_id, name)
);
CREATE TABLE connection_tool_discovery (
 connection_id UUID PRIMARY KEY REFERENCES connections(id) ON DELETE CASCADE,
 toolset_hash BYTEA NOT NULL,
 discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- Reachability samples for MCP servers added by URL, for their 12-hour history beside tool hosts.
-- A server is healthy while it answers an unauthenticated request below 500 (a 401 is a live
-- server asking for credentials); a timeout, refused connection or 5xx is a failed check.
CREATE TABLE mcp_server_health (
 id BIGSERIAL PRIMARY KEY,
 provider_id TEXT NOT NULL REFERENCES connection_providers(provider_id) ON DELETE CASCADE,
 checked_at TIMESTAMPTZ NOT NULL,
 healthy BOOLEAN NOT NULL
);
CREATE INDEX mcp_server_health_history ON mcp_server_health(provider_id, checked_at);
CREATE INDEX mcp_server_health_retention ON mcp_server_health(checked_at);

-- A dynamic agent keeps all its tools out of the listed catalog: it finds them with tools.search
-- and calls them through tools.execute.
ALTER TABLE agents ADD COLUMN tool_mode TEXT NOT NULL DEFAULT 'direct' CHECK (tool_mode IN ('direct','dynamic'));
-- An agent's use of one tool source: a tool-capable installation connection, or a tool host that
-- needs no credentials. The slug prefixes the catalog names of its tools (`slug.name`) for this
-- agent and appears in its tools_invoke grants, so it is fixed once the source is added.
CREATE TABLE agent_tool_sources (
 id UUID PRIMARY KEY,
 agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
 connection_id UUID REFERENCES connections(id) ON DELETE CASCADE,
 tool_host_id UUID REFERENCES tool_hosts(id) ON DELETE CASCADE,
 slug TEXT NOT NULL CHECK (slug ~ '^[a-z0-9][a-z0-9_-]{0,31}$'),
 CHECK (num_nonnulls(connection_id, tool_host_id) = 1),
 UNIQUE(agent_id, slug),
 UNIQUE(agent_id, connection_id),
 UNIQUE(agent_id, tool_host_id)
);
CREATE INDEX agent_tool_sources_by_connection ON agent_tool_sources(connection_id);
CREATE INDEX agent_tool_sources_by_tool_host ON agent_tool_sources(tool_host_id);
-- A tool the agent uses from its source. `name` is the catalog-safe form of the source's tool name.
CREATE TABLE agent_tools (
 source_id UUID NOT NULL REFERENCES agent_tool_sources(id) ON DELETE CASCADE,
 tool_name TEXT NOT NULL CHECK (tool_name <> ''),
 name TEXT NOT NULL CHECK (name ~ '^[A-Za-z0-9_-]{1,64}$'),
 is_async BOOLEAN NOT NULL DEFAULT false,
 -- The agent's own wording for the tool (empty keeps the tool's), and how its calls show in
 -- end-user chats; traces always keep the full call.
 summary TEXT NOT NULL DEFAULT '' CHECK (length(summary) <= 256),
 description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 4096),
 display TEXT NOT NULL DEFAULT 'full' CHECK (display IN ('full','summary','hidden')),
 PRIMARY KEY(source_id, tool_name),
 UNIQUE(source_id, name)
);
-- Bundled tools: tools an agent ships in its own code and runs in its process, registered per
-- invocation so search and schemas describe them beside server tools and the agent's Tools tab
-- shows the latest set. Definitions only: the engine never executes them. Rows go with the
-- invocation.
CREATE TABLE invocation_bundled_tools (
 invocation_id UUID NOT NULL REFERENCES chat_invocations(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK (name <> '' AND length(name) <= 128),
 description TEXT NOT NULL CHECK (description <> '' AND length(description) <= 4096),
 summary TEXT NOT NULL CHECK (length(summary) <= 256),
 input_schema_json TEXT NOT NULL,
 output_schema_json TEXT NOT NULL,
 read_only BOOLEAN NOT NULL,
 destructive BOOLEAN NOT NULL,
 idempotent BOOLEAN NOT NULL,
 open_world BOOLEAN NOT NULL,
 display TEXT NOT NULL CHECK (display IN ('full','summary','hidden')),
 PRIMARY KEY(invocation_id, name)
);
-- Finds an agent's latest invocation that registered bundled tools.
CREATE INDEX chat_invocations_by_agent_start ON chat_invocations(agent_id, started_at DESC);
-- Fixed when the call starts, so a later change never rewrites what a transcript showed. A
-- background (detached) call returns a ticket at once and its record stays running after the
-- invocation ends, so the interrupted-call sweep leaves it alone until it is plainly abandoned.
ALTER TABLE chat_tool_calls ADD COLUMN summary TEXT NOT NULL DEFAULT '' CHECK (length(summary) <= 256);
ALTER TABLE chat_tool_calls ADD COLUMN detached BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE chat_tool_calls ADD COLUMN display TEXT NOT NULL DEFAULT 'full' CHECK (display IN ('full','summary','hidden'));
