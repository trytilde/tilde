/**
 * Roles as the engine defines them. A role is a set of actions on an agent; actions do not
 * imply each other, so each role lists everything it gives. Every agent has these three system
 * roles, `agent/<id>/<slug>`; the `agents/<slug>` roles reach every agent and are for
 * administrators to assign.
 */
export const AGENT_ROLES = ["reader", "editor", "deployer"] as const;
export type AgentRole = (typeof AGENT_ROLES)[number];
export const roleActions: Record<AgentRole, readonly string[]> = {
  reader: ["view"],
  editor: ["view", "edit", "share"],
  deployer: ["view", "deploy", "share"],
};
export const roleLabels: Record<AgentRole, string> = {
  reader: "Reader",
  editor: "Editor",
  deployer: "Deployer",
};
/** The role id for one agent, or with an empty agent id for every agent. */
export function roleId(agentId: string, role: AgentRole): string {
  return agentId ? `agent/${agentId}/${role}` : `agents/${role}`;
}
export function roleSlug(id: string): AgentRole | undefined {
  const slug = id.slice(id.lastIndexOf("/") + 1);
  return AGENT_ROLES.find((role) => role === slug);
}
export function reachesEveryAgent(id: string): boolean {
  return id.startsWith("agents/");
}
/** Roles a caller holding `actions` may assign: `share`, plus every action the role gives. */
export function assignable(actions: readonly string[]): AgentRole[] {
  if (!actions.includes("share")) return [];
  return AGENT_ROLES.filter((role) => roleActions[role].every((a) => actions.includes(a)));
}
/** Every signed-in user. A reader role for it is what "anyone can view" means. */
export const EVERYONE_GROUP = "tilde_system:user";
