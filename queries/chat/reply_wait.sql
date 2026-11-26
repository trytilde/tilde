--: Record()

--! run (p1, p2) : Record
-- The agent's open `thread.wait_for_reply` call in the thread, if its run is still alive: the
-- next message routed to the agent there is its output.
SELECT t.id, t.invocation_id, t.participant_id, t.name, t.provider_id FROM chat_tool_calls t
JOIN chat_invocations i ON i.id = t.invocation_id
JOIN chat_runs r ON r.id = i.run_id AND r.status IN ('active', 'suspending', 'waiting')
WHERE t.thread_id = :p1 AND i.agent_id = :p2 AND t.name = 'thread.wait_for_reply' AND t.status = 'running'
ORDER BY t.created_at LIMIT 1;
