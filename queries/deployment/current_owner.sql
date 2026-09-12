SELECT participant_id,owner_instance_id,generation,stopped FROM participant_assignments WHERE thread_id=$1 AND agent_id=$2 LIMIT 1;
