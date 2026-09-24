--: Record()

--! run (p1, p2) : Record
SELECT external_id FROM chat_channel_receipts WHERE connection_id=:p1 AND external_id=:p2;
