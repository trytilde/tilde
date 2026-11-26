--: Record()

--! run (p1, p2) : Record
-- The outcomes of a checkpoint's pending calls, recorded in the run's thread.
SELECT id, name, provider_id, status, output_json, error FROM chat_tool_calls
WHERE id = ANY(:p1::UUID[]) AND thread_id = :p2 ORDER BY id;
