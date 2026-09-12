UPDATE chat_runs SET status='suspending' WHERE id=$1 AND status IN ('active','suspending') AND EXISTS(SELECT 1 FROM chat_invocations WHERE id=$2 AND run_id=$1 AND status='running');
