import { appendFile, readFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { spawn } from "node:child_process";

// Dependency-free Connect unary JSON client; GitHub provides the Node runtime.
// Error messages deliberately never include HTTP bodies, URLs, credentials or causes.
class ActionError extends Error {}
const env = process.env;
const input = (name, fallback = "") => env[`INPUT_${name.toUpperCase()}`]?.trim() || fallback;
const escapeCommand = (value) =>
  value.replaceAll("%", "%25").replaceAll("\r", "%0D").replaceAll("\n", "%0A");
const mask = (value) => {
  if (value) process.stdout.write(`::add-mask::${escapeCommand(value)}\n`);
};
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function baseUrl(value, name) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new ActionError(`${name} must be an absolute HTTP(S) URL.`);
  }
  if (
    !["https:", "http:"].includes(url.protocol) ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  ) {
    throw new ActionError(`${name} must be HTTP(S), without credentials, a query, or a fragment.`);
  }
  return url.href.replace(/\/+$/, "");
}
async function output(name, value) {
  const delimiter = `tilde_${randomUUID()}`;
  await appendFile(env.GITHUB_OUTPUT, `${name}<<${delimiter}\n${value}\n${delimiter}\n`);
}
async function rpc(url, token, method, body) {
  let response;
  try {
    response = await fetch(`${url}/tilde.management.v1.DeploymentService/${method}`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "Connect-Protocol-Version": "1",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: JSON.stringify(body),
      redirect: "error", // Never forward credentials through an API redirect.
      signal: AbortSignal.timeout(30_000),
    });
  } catch {
    throw new ActionError(
      `${method} could not reach Tilde or timed out. Check the API URL and runner network access.`,
    );
  }
  if (!response.ok)
    throw new ActionError(
      `${method} failed (HTTP ${response.status}). Check the API URL, credentials and deployment fields.`,
    );
  try {
    return await response.json();
  } catch {
    throw new ActionError(`${method} returned an invalid JSON response.`);
  }
}
/**
 * Run the caller's deploy command (normally `npx tilde deploy dist/index.js --json`, which also
 * registers the prompts and skills the code declares) and read its JSON result. Its stdout
 * holds the token, so it is parsed, never echoed; stderr (the inventory) passes through.
 */
async function runCommand(command, env) {
  const child = spawn(command, { shell: true, env, stdio: ["ignore", "pipe", "inherit"] });
  let stdout = "";
  child.stdout.on("data", (chunk) => {
    stdout += chunk;
  });
  const [code] = await new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("close", (...result) => resolve(result));
  }).catch(() => {
    throw new ActionError("command could not be started.");
  });
  if (code !== 0) throw new ActionError(`command failed (exit ${code}); see its output above.`);
  let result;
  try {
    result = JSON.parse(stdout.trim().split("\n").at(-1));
  } catch {
    throw new ActionError("command did not print deployment JSON; pass --json to tilde deploy.");
  }
  return {
    deployment: { id: result?.deploymentId },
    token: result?.token ?? undefined,
    created: result?.created,
  };
}
async function run() {
  // Optional: open-source Tilde's management API takes no credential; Tilde Cloud requires a
  // management API key. Explicit input wins; secrets must be passed by the caller.
  const apiToken = input("api-token", env.TILDE_API_KEY?.trim());
  mask(apiToken);
  if (!env.GITHUB_OUTPUT)
    throw new ActionError("GITHUB_OUTPUT is required; run this as a GitHub Action.");
  const url = baseUrl(input("url", env.TILDE_URL), "url / TILDE_URL");
  const uiUrl = baseUrl(input("ui-url", env.TILDE_UI_URL || url), "ui-url / TILDE_UI_URL");
  const agentId = input("agent-id", env.TILDE_AGENT_ID);
  if (!uuid.test(agentId || ""))
    throw new ActionError("agent-id / TILDE_AGENT_ID must be an agent UUID.");
  const type = input("type", "gateway");
  const targets = {
    gateway: "DEPLOYMENT_TARGET_GATEWAY",
    sidecar: "DEPLOYMENT_TARGET_SIDECAR",
    lambda: "DEPLOYMENT_TARGET_LAMBDA",
  };
  if (!Object.hasOwn(targets, type))
    throw new ActionError("type must be gateway, sidecar, or lambda.");
  const reference = input("function-arn");
  if (type === "lambda" ? !/^arn:aws:lambda:[^:]+:\d+:function:.+/.test(reference) : !!reference) {
    throw new ActionError(
      "function-arn is required for Lambda and must be omitted for Gateway/Sidecar.",
    );
  }
  const rotate = input("rotate-existing-token", "false");
  if (!["true", "false"].includes(rotate))
    throw new ActionError("rotate-existing-token must be true or false.");
  const outputToken = input("output-token", "true");
  if (!["true", "false"].includes(outputToken))
    throw new ActionError("output-token must be true or false.");
  if (outputToken === "false" && rotate === "true")
    throw new ActionError("rotate-existing-token requires output-token: true.");
  const externalId = input(
    "external-id",
    env.GITHUB_REPOSITORY && env.GITHUB_RUN_ID
      ? `${env.GITHUB_REPOSITORY}:${env.GITHUB_RUN_ID}`
      : "",
  );
  if (!externalId) throw new ActionError("external-id is required outside a GitHub workflow run.");
  const commitSha = input("commit-sha", env.GITHUB_SHA);
  if (commitSha && !/^[a-f0-9]+$/i.test(commitSha))
    throw new ActionError("commit-sha must be a hex SHA.");
  let head;
  if (env.GITHUB_EVENT_PATH) {
    try {
      head = JSON.parse(await readFile(env.GITHUB_EVENT_PATH, "utf8")).head_commit;
    } catch {
      throw new ActionError("Unable to read GitHub event metadata.");
    }
  }
  const body = {
    agentId,
    source: "DEPLOYMENT_SOURCE_CI",
    target: targets[type],
    externalId,
    repository: input("repository", env.GITHUB_REPOSITORY),
    commitSha,
    branch: input("branch", env.GITHUB_REF_NAME),
    label: input("label"),
    ...(reference ? { targetReference: reference } : {}),
    // Do not attach a push's message/author to a different explicitly supplied SHA.
    ...(head?.id === commitSha
      ? {
          commitMessage: head.message?.slice(0, 512),
          commitAuthor: head.author?.name?.slice(0, 512),
        }
      : {}),
  };
  const command = input("command");
  // The command registers the deployment itself from the same environment and GitHub metadata.
  const registered = command
    ? await runCommand(command, {
        ...env,
        TILDE_URL: url,
        TILDE_API_KEY: apiToken || undefined,
        TILDE_AGENT_ID: agentId,
      })
    : await rpc(url, apiToken, "RegisterDeployment", body);
  let token = registered.token;
  // Mask before any file command, summary or validation error can emit it.
  if (typeof token === "string") mask(token);
  const deploymentId = registered.deployment?.id;
  if (!uuid.test(deploymentId || ""))
    throw new ActionError("Tilde did not return a valid deployment ID.");
  if (typeof registered.created !== "boolean" && registered.created !== undefined)
    throw new ActionError("Tilde returned an invalid creation status.");
  const created = registered.created === true; // Protobuf JSON may omit false.
  const deploymentUrl = `${uiUrl}/agent/${agentId}/deployment`;
  await output("deployment-id", deploymentId);
  await output("deployment-url", deploymentUrl);
  await output("created", String(created));
  if (env.GITHUB_STEP_SUMMARY) {
    // Only identifiers and a page link, never a bearer credential.
    await appendFile(
      env.GITHUB_STEP_SUMMARY,
      `### Tilde deployment\n\nDeployment: \`${deploymentId}\`\n\n[Open deployments](<${deploymentUrl.replaceAll(">", "%3E").replaceAll("<", "%3C")}>)\n\nFor manual setup, open Tilde and select **Rotate token** on this deployment. This generates a replacement and invalidates the CI-issued token.\n`,
    );
  }
  if (outputToken === "false") {
    // Registration still issues a token server-side. Discard it rather than exporting
    // it to later steps. Manual users issue a replacement in the Tilde UI.
    token = undefined;
    process.stdout.write(
      "Tilde deployment registered. Token output is disabled; open the deployment link for manual setup.\n",
    );
    return;
  }
  if (!token && !created && rotate === "true") {
    const issued = await rpc(url, apiToken, "IssueDeploymentToken", { agentId, deploymentId });
    token = issued.token;
    if (typeof token === "string") mask(token);
  }
  if (!token && !created)
    throw new ActionError(
      "This release is already registered; its token cannot be read again. Reuse your stored token, rotate it in Tilde, or explicitly set rotate-existing-token: true to replace it.",
    );
  if (typeof token !== "string" || !token || /[\r\n]/.test(token))
    throw new ActionError("Tilde did not return a valid deployment token.");
  await output("deployment-token", token);
  process.stdout.write(
    "Tilde deployment registered. The token is masked and available only as a same-job step output.\n",
  );
}

run().catch((error) => {
  const message =
    error instanceof ActionError
      ? error.message
      : "Deployment registration failed. Check the runner output files and Tilde service; sensitive error details were omitted.";
  process.stdout.write(`::error::${escapeCommand(message)}\n`);
  process.exitCode = 1;
});
