--: Record()

--! run (p1, p2, p3?, p4) : Record
-- Membership is read after the caller acquired the thread lock.
WITH member AS MATERIALIZED (
    SELECT id, active FROM chat_participants WHERE thread_id = :p1 AND id = :p2
), enabled AS (
    INSERT INTO chat_typing(thread_id, participant_id, expires_at)
    SELECT :p1, :p2, :p3::TIMESTAMPTZ FROM member WHERE active AND :p3 IS NOT NULL
    ON CONFLICT(thread_id, participant_id) DO UPDATE SET expires_at = EXCLUDED.expires_at
), disabled AS (
    DELETE FROM chat_typing WHERE thread_id = :p1 AND participant_id = :p2
      AND :p3::TIMESTAMPTZ IS NULL AND EXISTS(SELECT 1 FROM member WHERE active)
), advanced AS (
    UPDATE chat_threads SET activity_sequence = :p4::BIGINT
    WHERE id = :p1 AND activity_sequence = :p4::BIGINT - 1 AND EXISTS(SELECT 1 FROM member WHERE active)
    RETURNING id
)
SELECT EXISTS(SELECT 1 FROM member) AS present,
       EXISTS(SELECT 1 FROM member WHERE active) AS active,
       EXISTS(SELECT 1 FROM advanced) AS recorded, txid_current() AS transaction_id;
