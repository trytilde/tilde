--: Record(history_through_message_id?)

--! run (p1, p2) : Record
SELECT text,history_through_message_id FROM chat_inputs WHERE origin_invocation_id=:p1 AND id=:p2;
