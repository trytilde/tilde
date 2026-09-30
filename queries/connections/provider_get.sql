--: Record(account_name_label?, icon_url?, instructions?, remote_endpoint?, remote_ui_url?, tool_host_id?)

--! run (p1) : Record
SELECT provider_id,name,account_name_label,icon_url,instructions,kind,categories,remote_endpoint,remote_ui_url,tool_host_id FROM connection_providers WHERE provider_id=:p1;
