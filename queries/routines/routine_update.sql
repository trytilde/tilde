--! run (id, name, prompt, enabled, schedule?, connection?, signal_type?, next_run_at?)
UPDATE routines SET name=:name,prompt=:prompt,enabled=:enabled,schedule=:schedule,connection_id=:connection,
 signal_type=:signal_type,next_run_at=:next_run_at,updated_at=NOW() WHERE id=:id;
