-- Internal retry check: ciphertext is never part of an Agent response.
SELECT id, name, endpoint_url, created_at, updated_at, capabilities AS "capabilities!: Json<Capabilities>", webhook_signing_key FROM agents WHERE id = $1;
