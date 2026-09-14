INSERT INTO agents(id,name,endpoint_url,webhook_signing_key,capabilities,avatar_seed,deployment_mode) VALUES($1,$2,NULL,$3,$4,$1,'sidecar') ON CONFLICT(id) DO NOTHING;
