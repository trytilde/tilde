SELECT owner_instance_id,generation,stopped FROM participant_assignments WHERE thread_id=$1 AND participant_id=$2 FOR UPDATE;
