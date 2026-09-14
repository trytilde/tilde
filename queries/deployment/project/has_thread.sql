SELECT EXISTS(SELECT 1 FROM chat_threads WHERE id=$1) AS "exists!";
