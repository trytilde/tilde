--! run (p1)
UPDATE connections SET status='failed',updated_at=now() WHERE id=:p1 AND status<>'ready' AND status<>'disconnected';
