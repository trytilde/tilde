UPDATE chat_messages SET status=$2 WHERE id=$1 AND status='streaming';
