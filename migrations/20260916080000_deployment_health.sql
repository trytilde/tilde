-- Deployments are addressed through connected instances, never a declared public URL.
ALTER TABLE agent_deployments DROP COLUMN endpoint_url;
CREATE OR REPLACE FUNCTION agent_deployment_create() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE dep UUID;
BEGIN
 INSERT INTO agent_deployment_settings(agent_id) VALUES(NEW.id);
 IF NEW.endpoint_url IS NOT NULL THEN
  dep:=gen_random_uuid();
  INSERT INTO agent_deployments(id,agent_id,source,target,label,traffic_weight) VALUES(dep,NEW.id,'manual','gateway','Initial',100);
  UPDATE agent_deployment_settings SET serving_deployment_id=dep WHERE agent_id=NEW.id;
 END IF;
 RETURN NEW;
END;
$$;
-- Editing the agent's descriptive endpoint no longer creates or promotes a deployment.
DROP TRIGGER agent_endpoint_deployment ON agents;
DROP FUNCTION agent_endpoint_deployment();
-- A NULL endpoint identifies an aggregate routing-health sample. Historical endpoint
-- probes and per-sidecar telemetry remain stored, but are not aggregate health evidence.
ALTER TABLE agent_health ALTER COLUMN endpoint_url DROP NOT NULL;
