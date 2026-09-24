--: Record()

--! run (p1) : Record
SELECT NOT EXISTS(SELECT 1 FROM chat_task_dependencies d JOIN chat_tasks t ON t.id=d.dependency_id WHERE d.task_id=:p1 AND t.status<>'completed') AS ready;
