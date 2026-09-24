-- Tilde never calls an agent. Hosts dial in with a deployment token, so agents carry no
-- endpoint and no wake-signing key, and health is sampled from deployment readiness only.
CREATE OR REPLACE FUNCTION agent_deployment_create() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO agent_deployment_settings(agent_id) VALUES(NEW.id);
 RETURN NEW;
END;
$$;
ALTER TABLE agents DROP COLUMN endpoint_url, DROP COLUMN webhook_signing_key;
-- Historical HTTP probe results described endpoints that no longer exist.
DELETE FROM agent_health WHERE endpoint_url IS NOT NULL AND sidecar_event_id IS NULL;
-- What remains in this column is the sidecar replica that produced a per-replica sample.
ALTER TABLE agent_health RENAME COLUMN endpoint_url TO instance_id;
