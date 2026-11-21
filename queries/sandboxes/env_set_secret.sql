--! run (p1, p2, p3)
-- The variable references the secret p3 instead of holding a value.
INSERT INTO sandbox_blueprint_env(blueprint_id,name,secret_id) VALUES(:p1,:p2,:p3)
ON CONFLICT(blueprint_id,name) DO UPDATE SET encrypted_value=NULL,secret_id=excluded.secret_id;
