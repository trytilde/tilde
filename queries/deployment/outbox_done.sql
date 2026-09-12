UPDATE sidecar_outbox SET delivered_at=NOW() WHERE id=$1;
