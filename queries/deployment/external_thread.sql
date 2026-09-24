--: Record()

--! run (p1, p2, p3) : Record
SELECT thread_id FROM chat_channel_threads WHERE connection_id=:p1 AND external_id=:p2 AND agent_id=:p3;
