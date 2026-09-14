UPDATE connection_agents SET access_mode=$3 WHERE connection_id=$1 AND agent_id=$2 AND capability='channel';
