UPDATE chat_runs SET status='failed' WHERE id=$1 AND status IN ('active','suspending');
