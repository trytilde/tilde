// Register a Gateway deployment for an agent and dial in with its token, as a real host does.
import { connectAgent, createManagementClient } from "../dist/index.js";
import { DeploymentSource, DeploymentTarget } from "../dist/management.js";

/** Resolves once the gateway acknowledged Watch; returns the connection to close. */
export async function connectAgentFixture({ url, agentId, options }) {
  const { token } = await createManagementClient({ baseUrl: url }).deployments.registerDeployment({
    agentId,
    source: DeploymentSource.MANUAL,
    target: DeploymentTarget.GATEWAY,
  });
  let connection;
  await new Promise((resolve) => {
    connection = connectAgent({
      ...options,
      gatewayUrl: url,
      deploymentToken: token,
      onRegistered: resolve,
    });
  });
  return connection;
}
