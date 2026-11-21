--! run (p1, p2, p3) : (version)
INSERT INTO memory_settings(connection_id,model,dims,version) VALUES(:p1,:p2,:p3,nextval('memory_settings_version'))
ON CONFLICT ON CONSTRAINT memory_settings_pkey DO UPDATE SET connection_id=EXCLUDED.connection_id,model=EXCLUDED.model,dims=EXCLUDED.dims,version=EXCLUDED.version
RETURNING version;
