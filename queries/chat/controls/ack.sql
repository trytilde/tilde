INSERT INTO invocation_control_receipts(invocation_id,command_id) VALUES($1,$2) ON CONFLICT DO NOTHING RETURNING command_id;
