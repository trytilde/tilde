UPDATE participant_assignments SET owner_instance_id=$3,generation=$4,stopped=$5,updated_at=NOW() WHERE thread_id=$1 AND participant_id=$2;
