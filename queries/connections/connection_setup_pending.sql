UPDATE connections SET status='requires_action',updated_at=now() WHERE id=$1 AND status<>'ready';
