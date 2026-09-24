import { ConnectionSetupService } from "@trytilde/contracts/tilde/setup/v1/connections_pb.js";
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { createServer as createHttpServer } from "node:http";
import { once } from "node:events";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { ConnectionsService } from "@trytilde/contracts/tilde/management/v1/connections_pb.js";
import { createConnectionProviderServer } from "../../sdk/ts/packages/sdk/dist/connection-provider.js";
const origin = process.env.CONNECTION_TEST_URL;
assert(origin);
const backendToken = "remote-provider-test-key-01234567890123456789";
const secret = "remote-private-api-key";
let calls = 0;
let release;
let started;
const entered = new Promise((resolve) => {
  started = resolve;
});
const pending = new Promise((resolve) => {
  release = resolve;
});
const backend = createConnectionProviderServer({
  backendToken,
  async handleSetup(request) {
    calls++;
    const draft = Object.fromEntries(request.draft.map((field) => [field.key, field.value]));
    if (request.isCallback) {
      assert.equal(draft.workspace, "personal");
      assert.equal(draft.confirmed, "yes");
      assert.equal(request.fields.find((field) => field.key === "code")?.value, "verified");
      return {
        result: {
          case: "complete",
          value: { fields: [{ key: "api_key", value: secret }], accountLabel: "Remote account" },
        },
      };
    }
    if (request.action === "slow") {
      started();
      await pending;
      return {
        result: { case: "complete", value: { fields: [{ key: "api_key", value: secret }] } },
      };
    }
    if (request.action === "prepare") {
      assert.equal(draft.workspace, "personal");
      return {
        draft: [{ key: "confirmed", value: "yes" }],
        result: { case: "awaitInput", value: {} },
      };
    }
    assert.equal(request.action, "authorize");
    return {
      result: {
        case: "awaitInput",
        value: {
          redirectUrl:
            request.callbackUrl +
            "?" +
            new URLSearchParams({ state: request.callbackState, code: "verified" }),
        },
      },
    };
  },
});
backend.listen(0, "127.0.0.1");
await once(backend, "listening");
let uiRequests = 0;
const ui = createHttpServer((request, response) => {
  assert.equal(request.headers.authorization, undefined);
  assert.equal(request.headers.cookie, undefined);
  uiRequests++;
  response.writeHead(200, { "content-type": "text/html" });
  response.end("<!doctype html><html><body>Remote provider iframe</body></html>");
});
ui.listen(0, "127.0.0.1");
await once(ui, "listening");
const transport = createConnectTransport({ baseUrl: origin });
const client = createClient(ConnectionsService, transport);
const broker = createClient(ConnectionSetupService, transport);
const providerId = "custom/remote-" + randomUUID();
try {
  await client.registerProvider({
    backendToken,
    provider: {
      id: providerId,
      name: "Remote fixture",
      categories: ["other"],
      kind: {
        case: "remote",
        value: {
          endpoint: `http://127.0.0.1:${backend.address().port}`,
          uiUrl: `http://127.0.0.1:${ui.address().port}/dist/`,
        },
      },
      connectionTypes: [
        { id: "account", name: "Account", credentialSource: { case: "custom", value: {} } },
      ],
    },
  });
  const catalog = await client.getProvider({ id: providerId });
  assert(
    !JSON.stringify(catalog, (_, v) => (typeof v === "bigint" ? String(v) : v)).includes(
      backendToken,
    ),
  );
  async function begin() {
    const connection = await client.startConnection({
      id: randomUUID(),
      name: "Remote account",
      providerId,
      typeId: "account",
    });
    const url = new URL(connection.brokeringUrl);
    const scope = {
      setupId: url.pathname.split("/").at(-1),
      connectionSetupToken: url.searchParams.get("connection_setup_token"),
    };
    return { connection, scope, state: (await broker.getSetup(scope)).state };
  }
  const setup = await begin();
  const page = await fetch(origin + setup.state.uiPath, {
    headers: {
      origin: "null",
      authorization: "Bearer management-only",
      cookie: "management=must-not-forward",
    },
  });
  assert.equal(page.status, 200);
  assert((await page.text()).includes("Remote provider iframe"));
  assert.equal(uiRequests, 1);
  let state = (
    await broker.saveDraft({
      ...setup.scope,
      actionId: setup.state.actionId,
      draft: [{ key: "workspace", value: "personal" }],
    })
  ).state;
  await assert.rejects(
    broker.saveDraft({ ...setup.scope, actionId: setup.state.actionId, draft: [] }),
  );
  state = (
    await broker.executeProviderAction({
      ...setup.scope,
      actionId: state.actionId,
      action: "prepare",
    })
  ).state;
  assert.equal(state.draft.find((field) => field.key === "confirmed").value, "yes");
  state = (
    await broker.executeProviderAction({
      ...setup.scope,
      actionId: state.actionId,
      action: "authorize",
    })
  ).state;
  await assert.rejects(
    broker.saveDraft({
      ...setup.scope,
      actionId: state.actionId,
      draft: [{ key: "workspace", value: "tampered" }],
    }),
  );
  const redirect = new URL(state.action.value.url);
  const callback = await fetch(redirect.origin + redirect.pathname, {
    method: "POST",
    headers: {
      origin: "https://provider.example",
      "content-type": "application/x-www-form-urlencoded",
    },
    body: redirect.searchParams,
    redirect: "manual",
  });
  assert.equal(callback.status, 303);
  const completed = (await broker.getSetup(setup.scope)).state;
  assert.equal(completed.action.case, "complete");
  assert.deepEqual(completed.draft, []);
  const connection = await client.getConnection({ id: setup.connection.connection.id });
  assert.equal(connection.connection.status, "ready");
  assert(
    !JSON.stringify(connection, (_, value) =>
      typeof value === "bigint" ? String(value) : value,
    ).includes(secret),
  );
  const cancelled = await begin();
  const running = broker.executeProviderAction({
    ...cancelled.scope,
    actionId: cancelled.state.actionId,
    action: "slow",
  });
  const rejected = assert.rejects(running);
  await entered;
  await broker.cancelSetup(cancelled.scope);
  release();
  await rejected;
  assert.equal((await broker.getSetup(cancelled.scope)).state.action.case, "cancelled");
  const badId = providerId + "-bad";
  await client.registerProvider({
    backendToken: "wrong-provider-token-012345678901234567890",
    provider: { ...catalog.provider, id: badId },
  });
  const bad = await client.startConnection({
    id: randomUUID(),
    name: "Bad key",
    providerId: badId,
    typeId: "account",
  });
  const badUrl = new URL(bad.brokeringUrl);
  const badScope = {
    setupId: badUrl.pathname.split("/").at(-1),
    connectionSetupToken: badUrl.searchParams.get("connection_setup_token"),
  };
  const badState = (await broker.getSetup(badScope)).state;
  const before = calls;
  await assert.rejects(
    broker.executeProviderAction({ ...badScope, actionId: badState.actionId, action: "prepare" }),
  );
  assert.equal(calls, before);
  console.log(JSON.stringify({ connectionId: setup.connection.connection.id, providerId }));
} finally {
  backend.closeAllConnections();
  backend.close();
  ui.closeAllConnections();
  ui.close();
}
