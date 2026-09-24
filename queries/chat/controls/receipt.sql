--: Record()

--! run (p1, p2) : Record
SELECT EXISTS(SELECT 1 FROM invocation_control_receipts WHERE invocation_id=:p1 AND command_id=:p2) AS exists;
