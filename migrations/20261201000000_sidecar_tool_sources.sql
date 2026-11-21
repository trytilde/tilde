-- Sidecars replicate the agent tools they run themselves (tool picks on connection sources and
-- the discovered tools of MCP-served connections), so changes to them push a new configuration
-- as connection and assignment changes do. Tool mode lives on agents, which already notifies.
CREATE TRIGGER sidecar_tool_sources_notify AFTER INSERT OR UPDATE OR DELETE ON agent_tool_sources
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_agent_tools_notify AFTER INSERT OR UPDATE OR DELETE ON agent_tools
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
CREATE TRIGGER sidecar_connection_tools_notify AFTER INSERT OR UPDATE OR DELETE ON connection_tools
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_sidecar_configuration');
