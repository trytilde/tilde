SELECT id, backend, wrapping_key_id, wrap_nonce, wrapped_key
FROM encryption_keys
WHERE singleton = TRUE;
