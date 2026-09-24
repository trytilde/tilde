-- Deployment execution type replaces the agent-wide mode. Preserve existing pins
-- before allowing Latest to consider deployments of every type.
UPDATE chat_participants p SET deployment_id=COALESCE(
 (SELECT n.deployment_id FROM thread_leases l JOIN agent_instances n ON n.agent_id=l.agent_id AND n.instance_id=l.instance_id WHERE l.thread_id=p.thread_id AND l.agent_id=p.agent_id),
 (SELECT i.deployment_id FROM chat_invocations i WHERE i.thread_id=p.thread_id AND i.agent_id=p.agent_id AND i.deployment_id IS NOT NULL ORDER BY i.started_at DESC NULLS LAST,i.id LIMIT 1),
 (SELECT s.serving_deployment_id FROM agent_deployment_settings s WHERE s.agent_id=p.agent_id)
) WHERE p.agent_id IS NOT NULL AND p.deployment_id IS NULL;

ALTER TABLE agent_deployments DROP CONSTRAINT agent_deployments_target_check;
ALTER TABLE agent_deployments DROP CONSTRAINT agent_deployments_check;
ALTER TABLE agent_deployments DROP CONSTRAINT agent_deployments_check1;
UPDATE agent_deployments SET target=CASE target WHEN 'direct' THEN 'gateway' WHEN 'aws_lambda' THEN 'lambda' ELSE target END;
ALTER TABLE agent_deployments ADD CONSTRAINT agent_deployments_target_check CHECK(target IN ('gateway','sidecar','lambda'));
ALTER TABLE agent_deployments ADD CONSTRAINT agent_deployments_endpoint_check CHECK(target<>'lambda' OR endpoint_url IS NULL);
ALTER TABLE agent_deployments ADD CONSTRAINT agent_deployments_reference_check CHECK((target='lambda')=(target_reference IS NOT NULL));
ALTER TABLE agents DROP CONSTRAINT agents_endpoint_required;
ALTER TABLE agents DROP COLUMN deployment_mode;
ALTER TABLE agents ADD CONSTRAINT agents_endpoint_optional CHECK(endpoint_url IS NULL OR length(trim(endpoint_url))>0);

CREATE OR REPLACE FUNCTION agent_deployment_create() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE dep UUID;
BEGIN
 INSERT INTO agent_deployment_settings(agent_id) VALUES(NEW.id);
 IF NEW.endpoint_url IS NOT NULL THEN
  dep:=gen_random_uuid();
  INSERT INTO agent_deployments(id,agent_id,source,target,endpoint_url,label,traffic_weight) VALUES(dep,NEW.id,'manual','gateway',NEW.endpoint_url,'Initial',100);
  UPDATE agent_deployment_settings SET serving_deployment_id=dep WHERE agent_id=NEW.id;
 END IF;
 RETURN NEW;
END;
$$;
CREATE OR REPLACE FUNCTION agent_endpoint_deployment() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE dep UUID; policy TEXT;
BEGIN
 IF current_setting('tilde.deployment_mirror',TRUE)='on' OR NEW.endpoint_url IS NULL THEN RETURN NEW; END IF;
 SELECT routing INTO policy FROM agent_deployment_settings WHERE agent_id=NEW.id FOR UPDATE;
 dep:=gen_random_uuid();
 INSERT INTO agent_deployments(id,agent_id,source,target,endpoint_url,label) VALUES(dep,NEW.id,'manual','gateway',NEW.endpoint_url,'Endpoint change');
 IF policy='latest' THEN
  UPDATE agent_deployments SET traffic_weight=CASE WHEN id=dep THEN 100 ELSE 0 END WHERE agent_id=NEW.id;
  UPDATE agent_deployment_settings SET serving_deployment_id=dep WHERE agent_id=NEW.id;
 END IF;
 RETURN NEW;
END;
$$;
UPDATE agent_deployment_settings s SET serving_deployment_id=(SELECT d.id FROM agent_deployments d WHERE d.agent_id=s.agent_id AND d.status='registered' ORDER BY d.created_at DESC,d.id LIMIT 1) WHERE s.routing='latest';
UPDATE agent_deployments d SET traffic_weight=CASE WHEN d.id=s.serving_deployment_id THEN 100 ELSE 0 END FROM agent_deployment_settings s WHERE s.agent_id=d.agent_id AND s.routing='latest';
SELECT set_config('tilde.deployment_mirror','on',true);
UPDATE agents a SET endpoint_url=(SELECT d.endpoint_url FROM agent_deployment_settings s JOIN agent_deployments d ON d.id=s.serving_deployment_id WHERE s.agent_id=a.id AND d.target='gateway');
