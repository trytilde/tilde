--: Record(remote_endpoint?, remote_ui_url?, remote_authorization?, remote_authorization_id?)

--! run (p1) : Record
SELECT kind,remote_endpoint,remote_ui_url,remote_authorization,remote_authorization_id FROM connection_providers WHERE provider_id=:p1;
