--: Record()

--! run (p1, p2, p3, p4, p5, p6?, p7, p8, p9, p10, p11, p12, p13) : Record
-- Complete native calls have no external effect. Winning the unique call ID gates
-- every message write; the claim and completed result commit atomically with it.
WITH claimed AS (
    INSERT INTO chat_tool_calls(id, thread_id, invocation_id, participant_id,
                                name, provider_id, status, input_json, output_json)
    VALUES(:p1, :p2, :p5, :p3, 'sendMessage', 'native', 'completed', :p12, :p13)
    ON CONFLICT(id) DO NOTHING RETURNING id
), created AS (
    INSERT INTO chat_messages(id, thread_id, participant_id, text, status, invocation_id,
                              in_reply_to_message_id, traceparent, tracestate)
    SELECT id, :p2, :p3, :p4, 'complete', :p5, :p6, :p9, :p10 FROM claimed
    RETURNING id
), targeted AS (
    INSERT INTO chat_message_targets(message_id, participant_id)
    SELECT created.id, p.id FROM created CROSS JOIN UNNEST(:p7::UUID[]) AS recipient
    JOIN chat_participants p ON p.id = recipient AND p.thread_id = :p2 AND p.active
    RETURNING participant_id
), attached AS (
    INSERT INTO chat_message_attachments(thread_id, message_id, attachment_id)
    SELECT :p2, created.id, attachment FROM created CROSS JOIN UNNEST(:p8::UUID[]) AS attachment
    RETURNING attachment_id
), advanced AS (
    UPDATE chat_threads SET activity_sequence = activity_sequence + :p11::BIGINT
    WHERE id = :p2 AND EXISTS(SELECT 1 FROM created)
    RETURNING activity_sequence - :p11::BIGINT + 1 AS sequence
)
SELECT advanced.sequence AS audit_sequence, txid_current() AS transaction_id,
       (SELECT COUNT(*) FROM targeted) AS target_count
FROM advanced;
