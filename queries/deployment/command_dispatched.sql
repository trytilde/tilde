UPDATE sidecar_commands SET dispatched_at=COALESCE(dispatched_at,NOW()) WHERE id=$1 AND generation=$2;
