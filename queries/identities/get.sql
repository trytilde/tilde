--: Record(connection_id?, root_identity_id?, verified_at?, attested_at?, attested_by_user_id?, attested_by_api_key_id?)
--! run (id) : Record
SELECT u.id,u.name,COALESCE(c.provider_id,'tilde') AS provider_id,i.connection_id,
 COALESCE(i.identity_type,'username') AS identity_type,COALESCE(i.value,n.value,u.id::TEXT) AS value,
 u.root_identity_id,i.verified_at,u.attested_at,u.attested_by_user_id,u.attested_by_api_key_id
FROM chat_users u LEFT JOIN chat_channel_identities i ON i.id=u.id
LEFT JOIN connections c ON c.id=i.connection_id LEFT JOIN chat_native_identities n ON n.id=u.id
WHERE u.id=:id;
