UPDATE sidecar_conversations SET storage='corrosion',retirement_epoch=NULL WHERE thread_id=$1 AND agent_id=$2 AND storage='retiring';
