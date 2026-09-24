-- Inference providers are ordinary connections: the capability names what an assignment grants.
ALTER TABLE connection_types ADD COLUMN inference_capable BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE connection_agents DROP CONSTRAINT connection_agents_capability_check;
ALTER TABLE connection_agents ADD CONSTRAINT connection_agents_capability_check CHECK (capability IN ('channel','inference'));
-- `provider_id/name` is the slug agents route by, so it must be unique once credentials work.
CREATE UNIQUE INDEX connection_slug ON connections(provider_id, name) WHERE status='ready';

-- One row per forwarded inference call, written off the request path. No foreign keys: usage
-- history outlives agents and connections, and sidecar records may arrive after either is gone.
CREATE TABLE inference_requests (
 id UUID PRIMARY KEY,
 created_at TIMESTAMPTZ NOT NULL,
 agent_id UUID NOT NULL,
 connection_id UUID NOT NULL,
 invocation_id UUID NOT NULL,
 thread_id UUID NOT NULL,
 run_id UUID NOT NULL,
 participant_id UUID NOT NULL,
 provider_id TEXT NOT NULL,
 kind TEXT NOT NULL,
 path TEXT NOT NULL,
 model TEXT,
 status INTEGER NOT NULL,
 latency_ms INTEGER NOT NULL,
 first_byte_ms INTEGER,
 request_bytes BIGINT NOT NULL,
 response_bytes BIGINT NOT NULL,
 input_tokens BIGINT,
 output_tokens BIGINT,
 cached_input_tokens BIGINT,
 -- parsed | missing | truncated | binary | error
 usage TEXT NOT NULL
);
CREATE INDEX inference_requests_agent ON inference_requests(agent_id, created_at DESC);
CREATE INDEX inference_requests_participant ON inference_requests(participant_id, created_at DESC);
