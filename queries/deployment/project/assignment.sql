INSERT INTO participant_assignments(thread_id,participant_id,agent_id,owner_instance_id,generation,stopped) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(thread_id,participant_id) DO NOTHING;
