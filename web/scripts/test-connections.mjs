import { ConnectionSetupService } from "../src/gen/tilde/setup/v1/connections_pb.js";
// Run from the Rust integration test against its isolated Postgres-backed Connect server.
import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { createClient } from "@connectrpc/connect";
import { createConnectTransport } from "@connectrpc/connect-web";
import { ConnectionsService } from "../src/gen/tilde/management/v1/connections_pb.js";
import { Capability, ClientAuthentication } from "../src/gen/tilde/types/v1/connections_pb.js";
const origin = process.env.CONNECTION_TEST_URL;
assert(origin, "CONNECTION_TEST_URL is required");
const transport = createConnectTransport({ baseUrl: origin });
const client = createClient(ConnectionsService, transport);
const broker = createClient(ConnectionSetupService, transport);
const catalog = await client.listProviders({ pageSize: 30 });
const github = catalog.providers.find((provider) => provider.id === "github");
assert.equal(github.connectionTypes.length, 1);
assert.equal(github.connectionTypes[0].credentialSource.case, "custom");
assert.deepEqual(github.connectionTypes[0].capabilities, [Capability.CHANNEL]);
const providerId = `custom/sdk-${randomUUID()}`;
await client.registerProvider({
  provider: {
    id: providerId,
    name: "SDK static provider",
    categories: ["other"],
    kind: { case: "configured", value: {} },
    connectionTypes: [
      {
        id: "static",
        name: "Static fields",
        credentialSource: {
          case: "static",
          value: {
            schemaJson: JSON.stringify({
              type: "object",
              additionalProperties: false,
              properties: {
                "random-key": { type: "string", title: "API key", writeOnly: true, minLength: 1 },
              },
              required: ["random-key"],
            }),
          },
        },
      },
    ],
  },
});
const connectionId = randomUUID();
const start = await client.startConnection({
  id: connectionId,
  name: "SDK account",
  providerId,
  typeId: "static",
});
assert(start.brokeringUrl);
assert.equal(start.connection.status, "requires_action");
const url = new URL(start.brokeringUrl);
const setupId = url.pathname.split("/").at(-1);
const connectionSetupToken = url.searchParams.get("connection_setup_token");
await assert.rejects(broker.getSetup({ setupId, connectionSetupToken: "wrong" }));
const view = await broker.getSetup({ setupId, connectionSetupToken });
assert.equal(view.state.action.case, "form");
const secret = "this-must-stay-out-of-all-responses";
const done = await broker.saveCredentials({
  setupId,
  connectionSetupToken,
  actionId: view.state.actionId,
  fields: [{ key: "random-key", value: secret }],
});
assert.equal(done.state.action.case, "complete");
const get = await client.getConnection({ id: connectionId });
assert.equal(get.connection.status, "ready");
assert.deepEqual(get.connection.capabilities, []);
assert(!JSON.stringify(get, (_, v) => (typeof v === "bigint" ? v.toString() : v)).includes(secret));
const oauthId = `custom/oauth-${randomUUID()}`;
await client.registerProvider({
  provider: {
    id: oauthId,
    name: "SDK OAuth provider",
    categories: ["other"],
    kind: { case: "configured", value: {} },
    connectionTypes: [
      {
        id: "oauth",
        name: "OAuth",
        credentialSource: {
          case: "oauth",
          value: {
            grant: 1,
            configuration: {
              authorizationUrl: process.env.CONNECTION_OAUTH_URL + "/authorize",
              tokenUrl: process.env.CONNECTION_OAUTH_URL + "/token",
              clientAuthentication: ClientAuthentication.BODY,
              pkce: true,
            },
          },
        },
      },
    ],
  },
});
const oauth = await client.startConnection({
  id: randomUUID(),
  name: "SDK OAuth",
  providerId: oauthId,
  typeId: "oauth",
});
const oauthBrokeringUrl = new URL(oauth.brokeringUrl);
const oauthSetup = oauthBrokeringUrl.pathname.split("/").at(-1);
const oauthConnectionSetupToken = oauthBrokeringUrl.searchParams.get("connection_setup_token");
const initial = await broker.getSetup({
  setupId: oauthSetup,
  connectionSetupToken: oauthConnectionSetupToken,
});
const consent = await broker.startOAuth({
  setupId: oauthSetup,
  connectionSetupToken: oauthConnectionSetupToken,
  actionId: initial.state.actionId,
  fields: [
    { key: "client_id", value: "client" },
    { key: "client_secret", value: "secret" },
  ],
});
const authorization = new URL(consent.state.action.value.url);
assert.equal(authorization.searchParams.get("redirect_uri"), origin + "/connections/callback");
const callback = await fetch(
  origin +
    "/connections/callback?" +
    new URLSearchParams({ state: authorization.searchParams.get("state"), code: "code" }),
  { redirect: "manual" },
);
assert.equal(callback.status, 303);
assert.equal(callback.headers.get("cache-control"), "no-store");
assert(callback.headers.get("location").startsWith(origin + "/connections/broker/"));
assert.equal(
  (
    await broker.getSetup({
      setupId: oauthSetup,
      connectionSetupToken: oauthConnectionSetupToken,
    })
  ).state.action.case,
  "complete",
);
await client.disconnect({ id: connectionId });
assert.equal((await client.getConnection({ id: connectionId })).connection.status, "disconnected");
console.log(
  "PASS: generated Connect clients, custom providers, capabilities, encrypted setup, and native OAuth callback",
);
