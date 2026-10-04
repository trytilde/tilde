--! run (id, name, prompt, thread_title, enabled, schedule?, connection?, signal_type?, next_run_at?)
UPDATE routines SET name=:name,prompt=:prompt,thread_title=:thread_title,enabled=:enabled,schedule=:schedule,connection_id=:connection,
 signal_type=:signal_type,next_run_at=:next_run_at,updated_at=NOW() WHERE id=:id;
