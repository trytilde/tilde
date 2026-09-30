-- Zero rows means the same batch is already pending in this queue.
--! run (object_key, queue, agent_id, batch_id, bytes)
INSERT INTO telemetry_objects(object_key,queue,agent_id,batch_id,bytes)
VALUES(:object_key,:queue,:agent_id,:batch_id,:bytes)
ON CONFLICT(queue,batch_id) DO NOTHING;
