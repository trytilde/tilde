UPDATE participant_assignments SET stopped=TRUE,generation=generation+1,updated_at=NOW() WHERE agent_id=$1;
