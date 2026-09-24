--: Record(authorization_url?, token_url?, success_path?, credential_schema?)

--! run (p1) : Record
SELECT provider_id,type_id,name,driver,channel_capable,authorization_url,token_url,client_auth,pkce,scopes,scope_separator,access_token_path,refresh_token_path,expires_in_path,scope_path,success_path,credential_schema,inference_capable FROM connection_types WHERE provider_id=:p1 ORDER BY type_id;
