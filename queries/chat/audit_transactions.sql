--: Record(status?)

--! run (p1) : Record
SELECT id AS transaction_id, pg_xact_status(id::TEXT::XID8) AS status
FROM UNNEST(:p1::BIGINT[]) AS id;
