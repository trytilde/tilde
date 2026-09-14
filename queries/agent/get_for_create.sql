-- Internal retry check: ciphertext is never part of an Agent response.
SELECT id, name, avatar_seed, avatar_key, paused, endpoint_url, created_at, updated_at, capabilities AS "capabilities!: Json<Capabilities>", webhook_signing_key FROM agents WHERE deleted_at IS NULL AND id = $1;
