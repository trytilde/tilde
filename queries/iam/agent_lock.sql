SELECT id, name, endpoint_url, created_at, updated_at, capabilities AS "capabilities!: Json<Capabilities>" FROM agents WHERE id = $1 FOR UPDATE;
