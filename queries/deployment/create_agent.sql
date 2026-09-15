INSERT INTO agents(id,name,endpoint_url,webhook_signing_key,capabilities,avatar_seed,deployment_mode,concurrency_policy) VALUES($1,$2,NULL,$3,$4,$1,'sidecar',$5) ON CONFLICT(id) DO NOTHING;
