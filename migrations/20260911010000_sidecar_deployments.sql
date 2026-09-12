ALTER TABLE agents ADD COLUMN deployment_mode TEXT NOT NULL DEFAULT 'gateway' CHECK(deployment_mode IN ('gateway','sidecar'));
ALTER TABLE agents DROP CONSTRAINT agents_endpoint_required;
ALTER TABLE agents ADD CONSTRAINT agents_endpoint_required CHECK (
 deleted_at IS NOT NULL OR
 (deployment_mode='gateway' AND endpoint_url IS NOT NULL AND length(trim(endpoint_url))>0) OR
 (deployment_mode='sidecar' AND endpoint_url IS NULL)
) NOT VALID;
CREATE TABLE agent_deployments (
 agent_id UUID PRIMARY KEY REFERENCES agents(id),
 failure_mode TEXT NOT NULL DEFAULT 'reassign' CHECK(failure_mode IN ('reassign','stop')),
 token_hash BYTEA,
 encrypted_secrets BYTEA
);
INSERT INTO agent_deployments(agent_id) SELECT id FROM agents;
CREATE FUNCTION agent_deployment_create() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO agent_deployments(agent_id) VALUES(NEW.id);
 RETURN NEW;
END;
$$;
CREATE TRIGGER agent_deployment_create AFTER INSERT ON agents FOR EACH ROW EXECUTE FUNCTION agent_deployment_create();
-- One row per sidecar process incarnation. Liveness is the heartbeat carried by Publish.
CREATE TABLE sidecar_nodes (
 agent_id UUID NOT NULL REFERENCES agents(id), instance_id UUID NOT NULL,
 public_url TEXT NOT NULL, runtime_url TEXT NOT NULL,
 ready BOOLEAN NOT NULL DEFAULT FALSE, agent_ready BOOLEAN NOT NULL DEFAULT FALSE,
 last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY(agent_id,instance_id)
);
-- The gateway is the only writer. Generations fence stale owners.
CREATE TABLE participant_assignments (
 thread_id UUID NOT NULL, participant_id UUID NOT NULL, agent_id UUID NOT NULL REFERENCES agents(id),
 owner_instance_id UUID NOT NULL, generation BIGINT NOT NULL CHECK(generation>0),
 stopped BOOLEAN NOT NULL DEFAULT FALSE, updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY(thread_id,participant_id)
);
CREATE INDEX participant_assignments_agent ON participant_assignments(agent_id,updated_at);
-- Durable downstream queue: gateway-originated work delivered over Watch, acknowledged by Publish.
CREATE TABLE sidecar_directives (
 id UUID PRIMARY KEY, agent_id UUID NOT NULL REFERENCES agents(id), instance_id UUID NOT NULL,
 thread_id UUID, generation BIGINT NOT NULL DEFAULT 0, payload BYTEA NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), acked_at TIMESTAMPTZ, result BYTEA
);
CREATE INDEX sidecar_directives_pending ON sidecar_directives(agent_id,instance_id,created_at) WHERE acked_at IS NULL;
-- Receipts deduplicate replayed events after reconnects; projected state lives in chat tables.
CREATE TABLE sidecar_events (
 event_id UUID PRIMARY KEY, agent_id UUID NOT NULL REFERENCES agents(id), thread_id UUID,
 origin_instance_id UUID NOT NULL, origin_sequence BIGINT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL, archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX sidecar_events_thread ON sidecar_events(thread_id,archived_at,event_id);
CREATE TRIGGER sidecar_directives_notify AFTER INSERT OR UPDATE ON sidecar_directives FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_directives');
CREATE TRIGGER sidecar_assignments_notify AFTER INSERT OR UPDATE ON participant_assignments FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_assignments');
CREATE TRIGGER sidecar_configuration_notify AFTER UPDATE ON agents FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_connections_notify AFTER INSERT OR UPDATE OR DELETE ON connections FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_deployment_notify AFTER UPDATE ON agent_deployments FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_channel_assignment_notify AFTER INSERT OR UPDATE OR DELETE ON connection_agents FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_identity_grant_notify AFTER INSERT OR UPDATE OR DELETE ON chat_channel_identity_access FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_identity_notify AFTER INSERT OR UPDATE OR DELETE ON chat_channel_identities FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_recovery_health AFTER INSERT OR UPDATE ON sidecar_nodes FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_recovery');
CREATE TRIGGER sidecar_recovery_agents AFTER UPDATE ON agents FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_recovery');
ALTER TABLE chat_attachments ADD COLUMN object_key TEXT;
ALTER TABLE agent_health ADD COLUMN sidecar_event_id UUID UNIQUE;
