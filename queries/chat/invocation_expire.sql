UPDATE chat_invocations SET status='failed',ended_at=NOW() WHERE status='running' AND lease_expires_at<NOW() RETURNING id,run_id,thread_id;
