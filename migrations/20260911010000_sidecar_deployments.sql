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
 retention_days BIGINT NOT NULL DEFAULT 7 CHECK(retention_days BETWEEN 1 AND 4294967295),
 token_hash BYTEA,
 encrypted_secrets BYTEA,
 cluster_id UUID NOT NULL DEFAULT gen_random_uuid()
);
INSERT INTO agent_deployments(agent_id) SELECT id FROM agents;
CREATE FUNCTION agent_deployment_create() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO agent_deployments(agent_id) VALUES(NEW.id);
 RETURN NEW;
END;
$$;
CREATE TRIGGER agent_deployment_create AFTER INSERT ON agents FOR EACH ROW EXECUTE FUNCTION agent_deployment_create();
CREATE TABLE sidecar_nodes (
 agent_id UUID NOT NULL REFERENCES agents(id), instance_id UUID NOT NULL,
 public_ingress_url TEXT NOT NULL, agent_ingress_url TEXT NOT NULL, runtime_url TEXT NOT NULL,
 gossip_address TEXT NOT NULL, local_agent_endpoint TEXT NOT NULL,
 ready BOOLEAN NOT NULL DEFAULT FALSE, agent_ready BOOLEAN NOT NULL DEFAULT FALSE,
 last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY(agent_id,instance_id)
);
CREATE TABLE participant_assignments (
 thread_id UUID NOT NULL, participant_id UUID NOT NULL, agent_id UUID NOT NULL REFERENCES agents(id),
 owner_instance_id UUID NOT NULL, generation BIGINT NOT NULL CHECK(generation>0),
 stopped BOOLEAN NOT NULL DEFAULT FALSE, updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY(thread_id,participant_id)
);
CREATE TABLE sidecar_conversations (
 thread_id UUID NOT NULL, agent_id UUID NOT NULL REFERENCES agents(id),
 storage TEXT NOT NULL DEFAULT 'corrosion' CHECK(storage IN ('corrosion','retiring','postgres')),
 last_activity_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), retired_at TIMESTAMPTZ, retirement_epoch UUID,
 PRIMARY KEY(thread_id,agent_id)
);
CREATE TABLE sidecar_commands (
 id UUID NOT NULL, generation BIGINT NOT NULL, invocation_id UUID, agent_id UUID NOT NULL REFERENCES agents(id),
 thread_id UUID NOT NULL, participant_id UUID NOT NULL, owner_instance_id UUID NOT NULL,
 kind TEXT NOT NULL, payload BYTEA NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 observed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), dispatched_at TIMESTAMPTZ, acked_at TIMESTAMPTZ, finished_at TIMESTAMPTZ, failure TEXT,
 PRIMARY KEY(id,generation)
);
CREATE TABLE sidecar_outbox (
 id UUID PRIMARY KEY, agent_id UUID NOT NULL REFERENCES agents(id), kind TEXT NOT NULL,
 payload BYTEA NOT NULL, created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), attempts INTEGER NOT NULL DEFAULT 0, delivered_at TIMESTAMPTZ
);
CREATE TABLE sidecar_archive (
 event_id UUID PRIMARY KEY, agent_id UUID NOT NULL REFERENCES agents(id), thread_id UUID,
 origin_instance_id UUID NOT NULL, origin_sequence BIGINT NOT NULL,
 kind TEXT NOT NULL, payload BYTEA NOT NULL, created_at TIMESTAMPTZ NOT NULL,
 archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX sidecar_archive_thread ON sidecar_archive(thread_id,archived_at,event_id);
CREATE TRIGGER sidecar_commands_notify AFTER INSERT OR UPDATE ON sidecar_commands FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_commands');
CREATE TRIGGER sidecar_outbox_notify AFTER INSERT ON sidecar_outbox FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_outbox');
CREATE TRIGGER sidecar_configuration_notify AFTER UPDATE ON agents FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_connections_notify AFTER INSERT OR UPDATE OR DELETE ON connections FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');

CREATE TRIGGER sidecar_deployment_notify AFTER UPDATE ON agent_deployments FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_registration_notify AFTER INSERT ON sidecar_nodes FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');

CREATE TRIGGER sidecar_channel_assignment_notify AFTER INSERT OR UPDATE OR DELETE ON connection_agents FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_identity_grant_notify AFTER INSERT OR UPDATE OR DELETE ON chat_channel_identity_access FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_identity_notify AFTER INSERT OR UPDATE OR DELETE ON chat_channel_identities FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');

ALTER TABLE chat_attachments ADD COLUMN object_key TEXT;

CREATE TRIGGER sidecar_invocation_work AFTER INSERT OR UPDATE OF status ON chat_invocations FOR EACH ROW EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_work');
CREATE TRIGGER sidecar_input_work AFTER INSERT OR UPDATE OF accepted ON chat_inputs FOR EACH ROW EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_work');

CREATE TRIGGER sidecar_ready_work AFTER UPDATE ON sidecar_nodes FOR EACH ROW
WHEN (NEW.ready AND NEW.agent_ready AND (NOT OLD.ready OR NOT OLD.agent_ready OR OLD.last_seen_at<NEW.last_seen_at-INTERVAL '15 seconds'))
EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_work');
ALTER TABLE agent_health ADD COLUMN sidecar_event_id UUID UNIQUE;

CREATE TABLE sidecar_bridge_receipts(thread_id UUID NOT NULL, sequence BIGINT NOT NULL, agent_id UUID NOT NULL, PRIMARY KEY(thread_id,sequence,agent_id));

CREATE TRIGGER sidecar_recovery_commands AFTER INSERT OR UPDATE ON sidecar_commands FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_recovery');
CREATE TRIGGER sidecar_recovery_health AFTER INSERT OR UPDATE ON sidecar_nodes FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_recovery');
CREATE TRIGGER sidecar_recovery_agents AFTER UPDATE ON agents FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_recovery');

-- Snapshot replay may arrive out of order across peer subscriptions. Historical
-- events stay append-only; each entity's current projection advances separately.
CREATE TABLE sidecar_projection_versions (
 agent_id UUID NOT NULL REFERENCES agents(id), entity_type TEXT NOT NULL, entity_id UUID NOT NULL,
 origin_instance_id UUID NOT NULL, origin_sequence BIGINT NOT NULL, created_at TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(agent_id,entity_type,entity_id)
);
