--: Record(participant_id?)

--! run (p1, p2, p3, p4) : Record
SELECT i.status,p.id AS participant_id FROM chat_invocations i LEFT JOIN chat_participants p ON p.thread_id=i.thread_id AND p.agent_id=i.agent_id AND p.active WHERE i.id=:p1 AND i.agent_id=:p2 AND i.thread_id=:p3 AND i.run_id=:p4;
