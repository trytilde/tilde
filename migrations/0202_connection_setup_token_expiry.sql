-- Existing links must obey the same lifetime as newly issued connection setup tokens.
UPDATE connection_setups SET expires_at=LEAST(expires_at,created_at+interval '10 minutes');
