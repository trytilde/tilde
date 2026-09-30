--: Record()

--! run (p1, p2, p3) : Record
-- A ticket is the detached call's ID; only the agent that made it, in the thread it was made, may read it.
SELECT t.name,t.status,t.output_json,t.error FROM chat_tool_calls t
JOIN chat_participants p ON p.id=t.participant_id
WHERE t.id=:p1 AND t.thread_id=:p2 AND p.agent_id=:p3 AND t.detached;
