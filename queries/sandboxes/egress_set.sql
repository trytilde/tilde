--! run (p1, p2, p3, p4, p5, p6)
-- Add or replace a blueprint's egress grant (blueprint, slug, connection, config, version, enabled).
INSERT INTO sandbox_blueprint_egress(blueprint_id,slug,connection_id,config,config_version,enabled)
VALUES(:p1,:p2,:p3,:p4,:p5,:p6)
ON CONFLICT(blueprint_id,slug) DO UPDATE SET connection_id=excluded.connection_id,config=excluded.config,
  config_version=excluded.config_version,enabled=excluded.enabled,updated_at=now();
