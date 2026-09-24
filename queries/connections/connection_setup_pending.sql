--! run (p1)
UPDATE connections SET status='requires_action',updated_at=now() WHERE id=:p1 AND status<>'ready';
