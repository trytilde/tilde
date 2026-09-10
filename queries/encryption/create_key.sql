INSERT INTO encryption_keys (id, backend, wrapping_key_id, wrap_nonce, wrapped_key)
VALUES ($1, $2, $3, $4, $5);
