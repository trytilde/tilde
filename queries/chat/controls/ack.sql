--: Record()

--! run (p1, p2) : Record
INSERT INTO invocation_control_receipts(invocation_id,command_id) VALUES(:p1,:p2) ON CONFLICT DO NOTHING RETURNING command_id;
