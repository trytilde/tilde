SELECT EXISTS(SELECT 1 FROM chat_invocations WHERE agent_id=$1 AND status IN ('pending','running')) AS "active!";
