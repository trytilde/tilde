-- The same provider/account name may be used by different agents. Only ready
-- assignments are routable; pending setup names may still be provisional.
DROP INDEX connection_slug;
CREATE INDEX connection_provider_name ON connections(provider_id,name);

-- Serialize route publication on the owning agents, including ready/rename and
-- assignment changes. Connection locks precede agent locks in application services.
CREATE FUNCTION check_connection_route_names(target UUID) RETURNS VOID LANGUAGE plpgsql AS $$
DECLARE locked_agent UUID;
BEGIN
 -- Advisory locks avoid upgrading the shared agent lock held by assignment validation.
 FOR locked_agent IN SELECT DISTINCT agent_id FROM connection_agents WHERE connection_id=target ORDER BY agent_id LOOP
  PERFORM pg_advisory_xact_lock(hashtextextended('tilde/provider-routes/' || locked_agent::TEXT,0));
 END LOOP;
 IF EXISTS (
  SELECT 1 FROM connection_agents mine
  JOIN connections own ON own.id=mine.connection_id AND own.status='ready'
  JOIN connection_agents other ON other.agent_id=mine.agent_id AND other.capability=mine.capability AND other.connection_id<>mine.connection_id
  JOIN connections theirs ON theirs.id=other.connection_id AND theirs.status='ready' AND theirs.provider_id=own.provider_id AND theirs.name=own.name
  WHERE mine.connection_id=target
 ) THEN
  RAISE EXCEPTION 'Provider/account name already assigned to this agent and capability'
   USING ERRCODE='23505', CONSTRAINT='connection_agent_slug';
 END IF;
END;
$$;
CREATE FUNCTION check_connection_route_change() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 PERFORM check_connection_route_names(NEW.id);
 RETURN NEW;
END;
$$;
CREATE FUNCTION check_assignment_route_change() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
 PERFORM 1 FROM connections WHERE id=NEW.connection_id FOR UPDATE;
 PERFORM check_connection_route_names(NEW.connection_id);
 RETURN NEW;
END;
$$;
CREATE TRIGGER connection_route_name AFTER UPDATE OF name,status ON connections FOR EACH ROW EXECUTE FUNCTION check_connection_route_change();
CREATE TRIGGER assignment_route_name AFTER INSERT OR UPDATE ON connection_agents FOR EACH ROW EXECUTE FUNCTION check_assignment_route_change();
