-- A connection in use cannot be disconnected or deleted: ConnectionService refuses, naming every
-- use (`queries/connections/usages.sql`), so nothing that uses one is left pointing at a dead
-- connection. Deleting a connection never unbinds its users either; what stays a cascade is what
-- the connection owns (its values, setups, grants, discovered tools and skills).
ALTER TABLE connection_agents
    DROP CONSTRAINT connection_agents_connection_id_fkey,
    ADD CONSTRAINT connection_agents_connection_id_fkey
        FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE RESTRICT;
ALTER TABLE personal_connection_agents
    DROP CONSTRAINT personal_connection_agents_connection_id_fkey,
    ADD CONSTRAINT personal_connection_agents_connection_id_fkey
        FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE RESTRICT;
ALTER TABLE agent_tool_sources
    DROP CONSTRAINT agent_tool_sources_connection_id_fkey,
    ADD CONSTRAINT agent_tool_sources_connection_id_fkey
        FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE RESTRICT;
ALTER TABLE routines
    DROP CONSTRAINT routines_connection_id_fkey,
    ADD CONSTRAINT routines_connection_id_fkey
        FOREIGN KEY (connection_id) REFERENCES connections(id) ON DELETE RESTRICT;

-- Uses of connections disconnected before now stay: reconnecting one finds them as they were, and
-- they are listed (and hold it) like any other. Writing such a use again (an idempotent apply)
-- is refused until the connection is reconnected.
-- A deleted agent (its row stays) uses nothing: its tool sources, routines and personal
-- allowances go with it from now on, and those of agents deleted before go here.
DELETE FROM agent_tool_sources WHERE agent_id IN (SELECT id FROM agents WHERE deleted_at IS NOT NULL);
DELETE FROM routines WHERE agent_id IN (SELECT id FROM agents WHERE deleted_at IS NOT NULL);
DELETE FROM personal_connection_agents WHERE agent_id IN (SELECT id FROM agents WHERE deleted_at IS NOT NULL);

-- Putting a connection to use waits for a disconnect of it in progress (which holds the
-- connection's row FOR UPDATE while it looks for uses), then refuses it if it was disconnected:
-- a new use and a disconnect never both commit. The lock is the one a foreign key check takes,
-- so it waits for nothing else.
CREATE FUNCTION connection_usable() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE s TEXT;
BEGIN
    IF NEW.connection_id IS NULL
        OR (TG_OP = 'UPDATE' AND NEW.connection_id IS NOT DISTINCT FROM OLD.connection_id) THEN
        RETURN NEW;
    END IF;
    SELECT status INTO s FROM connections WHERE id = NEW.connection_id FOR KEY SHARE;
    IF s = 'disconnected' THEN
        RAISE EXCEPTION 'The connection is disconnected: reconnect it before using it'
            USING ERRCODE = 'TL001';
    END IF;
    RETURN NEW;
END $$;
DO $$
DECLARE t TEXT;
BEGIN
    FOR t IN SELECT unnest(ARRAY['connection_agents', 'personal_connection_agents', 'agent_tool_sources',
        'routines', 'sandbox_blueprints', 'sandbox_blueprint_egress', 'sandboxes', 'memory_settings', 'memory_synthesis']) LOOP
        EXECUTE format('CREATE TRIGGER %I BEFORE INSERT OR UPDATE OF connection_id ON %I FOR EACH ROW EXECUTE FUNCTION connection_usable()', t || '_usable', t);
    END LOOP;
END $$;
