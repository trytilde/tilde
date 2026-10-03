--! run (id, agent, name, prompt, enabled, schedule?, connection?, signal_type?, next_run_at?)
INSERT INTO routines(id,agent_id,name,prompt,enabled,schedule,connection_id,signal_type,next_run_at)
VALUES(:id,:agent,:name,:prompt,:enabled,:schedule,:connection,:signal_type,:next_run_at);
