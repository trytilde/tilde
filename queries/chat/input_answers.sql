--: Record()

--! run (p1) : Record
-- Messages queued on the invocation that its loop was never shown: on the gateway they wait for
-- the invocation's end (see `suspend_run`, which answers a reply wait with them).
SELECT i.id, i.text FROM chat_inputs i
WHERE i.invocation_id = :p1 AND NOT i.accepted ORDER BY i.queue_order, i.sequence;
