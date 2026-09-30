-- What each agent holds pending per telemetry queue, kept by statement triggers so admission
-- reads a handful of rows under its queue lock instead of aggregating every pending pointer.
-- A full queue otherwise turned each rejected batch into a queue-wide scan that held a
-- pooled connection behind the lock, starving the rest of the engine of connections.
CREATE TABLE telemetry_usage (
 queue TEXT NOT NULL,
 agent_id TEXT NOT NULL,
 bytes BIGINT NOT NULL,
 objects BIGINT NOT NULL,
 PRIMARY KEY (queue, agent_id)
);
INSERT INTO telemetry_usage(queue,agent_id,bytes,objects)
SELECT queue,agent_id,SUM(bytes),COUNT(*) FROM telemetry_objects GROUP BY queue,agent_id;

CREATE FUNCTION telemetry_usage_added() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO telemetry_usage(queue,agent_id,bytes,objects)
 SELECT queue,agent_id,SUM(bytes),COUNT(*) FROM added GROUP BY queue,agent_id ORDER BY queue,agent_id
 ON CONFLICT (queue,agent_id) DO UPDATE
 SET bytes=telemetry_usage.bytes+EXCLUDED.bytes, objects=telemetry_usage.objects+EXCLUDED.objects;
 RETURN NULL;
END $$;
-- Rows are touched in key order so concurrent multi-agent deletes cannot deadlock.
CREATE FUNCTION telemetry_usage_removed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 WITH gone AS (
  SELECT queue,agent_id,SUM(bytes) AS bytes,COUNT(*) AS objects FROM removed
  GROUP BY queue,agent_id ORDER BY queue,agent_id
 )
 UPDATE telemetry_usage u SET bytes=u.bytes-g.bytes, objects=u.objects-g.objects
 FROM gone g WHERE u.queue=g.queue AND u.agent_id=g.agent_id;
 DELETE FROM telemetry_usage u USING (SELECT DISTINCT queue,agent_id FROM removed) r
 WHERE u.queue=r.queue AND u.agent_id=r.agent_id AND u.objects<=0;
 RETURN NULL;
END $$;
CREATE TRIGGER telemetry_usage_added AFTER INSERT ON telemetry_objects
 REFERENCING NEW TABLE AS added FOR EACH STATEMENT EXECUTE FUNCTION telemetry_usage_added();
CREATE TRIGGER telemetry_usage_removed AFTER DELETE ON telemetry_objects
 REFERENCING OLD TABLE AS removed FOR EACH STATEMENT EXECUTE FUNCTION telemetry_usage_removed();
