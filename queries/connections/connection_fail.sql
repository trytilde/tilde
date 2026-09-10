UPDATE connections SET status='failed',updated_at=now() WHERE id=$1 AND status<>'ready' AND status<>'disconnected';
