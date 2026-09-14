-- An agent has many deployments: the declared half of "what is running", created by
-- CI or by hand and tied to a commit. Instances are the observed half: whatever dials
-- in with a deployment's token. Policy (capabilities, connections, IAM) stays on the
-- agent. Execution mode (gateway or sidecar) also stays on the agent for now; every
-- deployment of an agent shares it.
ALTER TABLE agent_deployments RENAME TO agent_deployment_settings;
ALTER TABLE agent_deployment_settings ADD COLUMN routing TEXT NOT NULL DEFAULT 'latest' CHECK(routing IN ('latest','manual'));
ALTER TABLE agent_deployment_settings ADD COLUMN serving_deployment_id UUID;

CREATE TABLE agent_deployments (
 id UUID PRIMARY KEY, agent_id UUID NOT NULL REFERENCES agents(id),
 source TEXT NOT NULL CHECK(source IN ('ci','manual')),
 target TEXT NOT NULL CHECK(target IN ('direct','sidecar','aws_lambda')),
 endpoint_url TEXT, target_reference TEXT,
 repository TEXT, commit_sha TEXT, external_id TEXT, label TEXT NOT NULL DEFAULT '',
 status TEXT NOT NULL DEFAULT 'registered' CHECK(status IN ('registered','retired')),
 token_hash BYTEA UNIQUE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), retired_at TIMESTAMPTZ,
 CHECK((target='direct') = (endpoint_url IS NOT NULL)),
 CHECK((target='aws_lambda') = (target_reference IS NOT NULL))
);
CREATE UNIQUE INDEX agent_deployments_external ON agent_deployments(agent_id,external_id) WHERE external_id IS NOT NULL;
CREATE INDEX agent_deployments_agent ON agent_deployments(agent_id,created_at DESC);
ALTER TABLE agent_deployment_settings ADD FOREIGN KEY(serving_deployment_id) REFERENCES agent_deployments(id);

-- Every existing agent gets one manual deployment carrying its current endpoint or token.
INSERT INTO agent_deployments(id,agent_id,source,target,endpoint_url,token_hash,label)
SELECT gen_random_uuid(),a.id,'manual',CASE WHEN a.deployment_mode='sidecar' THEN 'sidecar' ELSE 'direct' END,
 CASE WHEN a.deployment_mode='sidecar' THEN NULL ELSE a.endpoint_url END,
 CASE WHEN a.deployment_mode='sidecar' THEN s.token_hash END,'Initial'
FROM agents a JOIN agent_deployment_settings s ON s.agent_id=a.id
WHERE a.deleted_at IS NULL AND (a.deployment_mode='sidecar' OR a.endpoint_url IS NOT NULL);
UPDATE agent_deployment_settings s SET serving_deployment_id=d.id FROM agent_deployments d WHERE d.agent_id=s.agent_id;
ALTER TABLE agent_deployment_settings DROP COLUMN token_hash;

-- Instances: one row per process incarnation per agent, under one deployment.
ALTER TABLE sidecar_nodes RENAME TO agent_instances;
ALTER TABLE agent_instances ADD COLUMN deployment_id UUID REFERENCES agent_deployments(id);
UPDATE agent_instances i SET deployment_id=s.serving_deployment_id FROM agent_deployment_settings s WHERE s.agent_id=i.agent_id;
DELETE FROM agent_instances WHERE deployment_id IS NULL;
ALTER TABLE agent_instances ALTER COLUMN deployment_id SET NOT NULL;
CREATE INDEX agent_instances_deployment ON agent_instances(deployment_id,last_seen_at);

-- A (thread, agent) pair is pinned to a deployment on its first invocation; failover
-- stays within the deployment, moving across deployments is an explicit act.
ALTER TABLE chat_participants ADD COLUMN deployment_id UUID REFERENCES agent_deployments(id);
ALTER TABLE chat_invocations ADD COLUMN deployment_id UUID REFERENCES agent_deployments(id);
CREATE INDEX chat_invocations_deployment ON chat_invocations(deployment_id,id) WHERE deployment_id IS NOT NULL;

-- New agents get settings, and a manual direct deployment when created with an endpoint.
-- Promotion mirrors the serving endpoint back onto agents.endpoint_url with the flag set,
-- so legacy readers keep working without the trigger minting another deployment.
CREATE OR REPLACE FUNCTION agent_deployment_create() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE dep UUID;
BEGIN
 INSERT INTO agent_deployment_settings(agent_id) VALUES(NEW.id);
 IF NEW.endpoint_url IS NOT NULL THEN
  dep:=gen_random_uuid();
  INSERT INTO agent_deployments(id,agent_id,source,target,endpoint_url,label) VALUES(dep,NEW.id,'manual','direct',NEW.endpoint_url,'Initial');
  UPDATE agent_deployment_settings SET serving_deployment_id=dep WHERE agent_id=NEW.id;
 END IF;
 RETURN NEW;
END;
$$;
CREATE FUNCTION agent_endpoint_deployment() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE dep UUID;
BEGIN
 IF current_setting('tilde.deployment_mirror',TRUE)='on' OR NEW.endpoint_url IS NULL OR NEW.deployment_mode<>'gateway' THEN RETURN NEW; END IF;
 dep:=gen_random_uuid();
 INSERT INTO agent_deployments(id,agent_id,source,target,endpoint_url,label) VALUES(dep,NEW.id,'manual','direct',NEW.endpoint_url,'Endpoint change');
 UPDATE agent_deployment_settings SET serving_deployment_id=dep WHERE agent_id=NEW.id;
 RETURN NEW;
END;
$$;
CREATE TRIGGER agent_endpoint_deployment AFTER UPDATE OF endpoint_url ON agents FOR EACH ROW WHEN (OLD.endpoint_url IS DISTINCT FROM NEW.endpoint_url) EXECUTE FUNCTION agent_endpoint_deployment();
CREATE TRIGGER agent_deployments_notify AFTER INSERT OR UPDATE ON agent_deployments FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
