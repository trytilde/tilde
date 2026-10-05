--! run (p1, p2, p3)
INSERT INTO sandbox_blueprint_env(blueprint_id,name,encrypted_value) VALUES(:p1,:p2,:p3)
ON CONFLICT(blueprint_id,name) DO UPDATE SET encrypted_value=excluded.encrypted_value;
