--: Record()

--! run (p1) : Record
UPDATE chat_threads SET activity_sequence=activity_sequence+1 WHERE id=:p1 RETURNING activity_sequence, txid_current() AS transaction_id;
