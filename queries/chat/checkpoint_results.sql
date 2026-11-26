--: Record()

--! run (p1, p2) : Record
-- The records of calls made by the run's own invocations: a checkpoint names nothing else.
SELECT t.id, t.name, t.provider_id, t.status, t.output_json, t.error FROM chat_tool_calls t
JOIN chat_invocations i ON i.id = t.invocation_id AND i.run_id = :p2
WHERE t.id = ANY(:p1::UUID[]) ORDER BY t.id;
