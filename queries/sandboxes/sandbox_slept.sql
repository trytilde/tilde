--! run (p1, p2?, p3?)
-- Asleep: E2B keeps its paused VM (p2); Modal keeps only the filesystem snapshot (p3).
UPDATE sandboxes SET status='sleeping',provider_sandbox_id=:p2,snapshot_id=:p3,connected_at=NULL,
  session_hash=CASE WHEN :p2::TEXT IS NULL THEN NULL ELSE session_hash END,lease_until=NULL WHERE id=:p1;
