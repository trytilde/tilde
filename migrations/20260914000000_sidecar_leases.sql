-- Execution exclusivity moves from per-event ownership generations to one lease per
-- (thread, agent). Data appends from any replica are accepted; only run state is
-- checked against the lease. Lease validity is the holder's liveness in sidecar_nodes.
DROP TABLE participant_assignments;
CREATE TABLE thread_leases (
 thread_id UUID NOT NULL, agent_id UUID NOT NULL REFERENCES agents(id),
 instance_id UUID NOT NULL, updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY(thread_id,agent_id)
);
CREATE INDEX thread_leases_agent ON thread_leases(agent_id,updated_at);
CREATE INDEX thread_leases_instance ON thread_leases(agent_id,instance_id);
CREATE TRIGGER sidecar_leases_notify AFTER INSERT OR UPDATE OR DELETE ON thread_leases FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_leases');
ALTER TABLE sidecar_directives DROP COLUMN generation;
-- Recovery scans active invocations of dead lease holders.
CREATE INDEX chat_invocations_active_agent ON chat_invocations(agent_id,thread_id) WHERE status IN ('pending','running');
