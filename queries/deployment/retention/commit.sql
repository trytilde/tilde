UPDATE sidecar_conversations SET storage='postgres',retired_at=NOW() WHERE thread_id=$1 AND agent_id=$2 AND storage='retiring';
