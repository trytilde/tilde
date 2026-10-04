--: Record(schedule?, next_run_at?)

-- Locks due cron routines for the caller's transaction, which reschedules each before
-- committing; other replicas skip them meanwhile. Agents are soft-deleted, so a deleted
-- agent's routines are skipped here rather than cascaded away.
--! run (limit) : Record
SELECT r.id,r.agent_id,r.name,r.prompt,r.thread_title,r.schedule,r.next_run_at FROM routines r
JOIN agents a ON a.id=r.agent_id AND a.deleted_at IS NULL
WHERE r.next_run_at<=NOW() ORDER BY r.next_run_at LIMIT :limit FOR UPDATE OF r SKIP LOCKED;
