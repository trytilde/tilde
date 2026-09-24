--: Record(source_identity_id?)

--! run (p1, p2) : Record
SELECT id,source_identity_id,channel_origin FROM chat_runs WHERE thread_id=:p1 AND agent_id=:p2 AND status IN ('active','suspending','waiting');
