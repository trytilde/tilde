--: Record(authorization_url?, token_url?, success_path?, credential_schema?, mcp_url?, mcp_credential?, mcp_credential_name?)

--! run (p1) : Record
SELECT provider_id,type_id,name,driver,channel_capable,authorization_url,token_url,client_auth,pkce,scopes,scope_separator,access_token_path,refresh_token_path,expires_in_path,scope_path,success_path,credential_schema,inference_capable,tool_capable,mcp_url,mcp_credential,mcp_credential_name,mcp_credential_prefix,oauth_client,signal_capable FROM connection_types WHERE provider_id=:p1 ORDER BY type_id;
