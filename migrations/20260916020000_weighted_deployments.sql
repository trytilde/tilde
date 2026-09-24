-- Preserve the currently promoted deployment as 100% when replacing manual routing.
ALTER TABLE agent_deployments ADD COLUMN traffic_weight INTEGER NOT NULL DEFAULT 0 CHECK (traffic_weight BETWEEN 0 AND 100);
UPDATE agent_deployments d SET traffic_weight=100 FROM agent_deployment_settings s WHERE s.serving_deployment_id=d.id;
ALTER TABLE agent_deployment_settings DROP CONSTRAINT agent_deployment_settings_routing_check;
UPDATE agent_deployment_settings SET routing='weighted' WHERE routing='manual';
ALTER TABLE agent_deployment_settings ADD CONSTRAINT agent_deployment_settings_routing_check CHECK (routing IN ('latest','weighted'));

-- Initial and legacy endpoint-created deployments participate in the same policy.
CREATE OR REPLACE FUNCTION agent_deployment_create() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE dep UUID;
BEGIN
 INSERT INTO agent_deployment_settings(agent_id) VALUES(NEW.id);
 IF NEW.endpoint_url IS NOT NULL THEN
  dep:=gen_random_uuid();
  INSERT INTO agent_deployments(id,agent_id,source,target,endpoint_url,label,traffic_weight) VALUES(dep,NEW.id,'manual','direct',NEW.endpoint_url,'Initial',100);
  UPDATE agent_deployment_settings SET serving_deployment_id=dep WHERE agent_id=NEW.id;
 END IF;
 RETURN NEW;
END;
$$;
CREATE OR REPLACE FUNCTION agent_endpoint_deployment() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE dep UUID; policy TEXT;
BEGIN
 IF current_setting('tilde.deployment_mirror',TRUE)='on' OR NEW.endpoint_url IS NULL OR NEW.deployment_mode<>'gateway' THEN RETURN NEW; END IF;
 SELECT routing INTO policy FROM agent_deployment_settings WHERE agent_id=NEW.id FOR UPDATE;
 dep:=gen_random_uuid();
 INSERT INTO agent_deployments(id,agent_id,source,target,endpoint_url,label) VALUES(dep,NEW.id,'manual','direct',NEW.endpoint_url,'Endpoint change');
 IF policy='latest' THEN
  UPDATE agent_deployments SET traffic_weight=CASE WHEN id=dep THEN 100 ELSE 0 END WHERE agent_id=NEW.id;
  UPDATE agent_deployment_settings SET serving_deployment_id=dep WHERE agent_id=NEW.id;
 END IF;
 RETURN NEW;
END;
$$;

-- A gateway-mode agent may serve Lambda deployments or have no deployment yet.
-- Its legacy mirrored endpoint is therefore optional; sidecars still never have one.
ALTER TABLE agents DROP CONSTRAINT agents_endpoint_required;
ALTER TABLE agents ADD CONSTRAINT agents_endpoint_required CHECK (
 deleted_at IS NOT NULL OR
 (deployment_mode='gateway' AND (endpoint_url IS NULL OR length(trim(endpoint_url))>0)) OR
 (deployment_mode='sidecar' AND endpoint_url IS NULL)
);
