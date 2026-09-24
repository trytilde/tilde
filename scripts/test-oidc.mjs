// Local signed OIDC fixture for transport integration tests. No authentication bypass in Tilde.
import { createServer } from "node:http";
import { generateKeyPairSync, createHash, randomBytes, sign } from "node:crypto";
import { once } from "node:events";
import assert from "node:assert/strict";
export async function startOidc() {
  const { privateKey, publicKey } = generateKeyPairSync("rsa", { modulusLength: 2048 });
  const jwk = { ...publicKey.export({ format: "jwk" }), kid: "fixture", alg: "RS256", use: "sig" };
  const codes = new Map();
  let issuer;
  const server = createServer(async (req, res) => {
    const url = new URL(req.url, issuer);
    const json = (value) => {
      res.setHeader("Content-Type", "application/json");
      res.end(JSON.stringify(value));
    };
    if (url.pathname === "/.well-known/openid-configuration")
      return json({
        issuer,
        authorization_endpoint: `${issuer}/authorize`,
        token_endpoint: `${issuer}/token`,
        jwks_uri: `${issuer}/keys`,
      });
    if (url.pathname === "/keys") return json({ keys: [jwk] });
    if (url.pathname === "/authorize") {
      const code = randomBytes(24).toString("hex");
      codes.set(code, Object.fromEntries(url.searchParams));
      const redirect = new URL(url.searchParams.get("redirect_uri"));
      redirect.searchParams.set("code", code);
      redirect.searchParams.set("state", url.searchParams.get("state"));
      res.writeHead(302, { Location: redirect.toString() });
      return res.end();
    }
    if (url.pathname === "/token") {
      let body = "";
      for await (const chunk of req) body += chunk;
      const form = new URLSearchParams(body);
      const auth = codes.get(form.get("code"));
      codes.delete(form.get("code"));
      if (
        !auth ||
        req.headers.authorization !==
          `Basic ${Buffer.from("test-client:test-secret").toString("base64")}` ||
        auth.redirect_uri !== form.get("redirect_uri") ||
        auth.code_challenge !==
          createHash("sha256")
            .update(form.get("code_verifier") ?? "")
            .digest("base64url")
      ) {
        res.statusCode = 400;
        return json({ error: "invalid_grant" });
      }
      const now = Math.floor(Date.now() / 1000);
      const head = Buffer.from(JSON.stringify({ alg: "RS256", kid: "fixture" })).toString(
        "base64url",
      );
      const claims = Buffer.from(
        JSON.stringify({
          iss: issuer,
          aud: "test-client",
          sub: "test-user",
          nonce: auth.nonce,
          iat: now,
          exp: now + 300,
        }),
      ).toString("base64url");
      const data = `${head}.${claims}`;
      return json({
        id_token: `${data}.${sign("RSA-SHA256", Buffer.from(data), privateKey).toString("base64url")}`,
        access_token: "opaque-provider-token",
        token_type: "Bearer",
      });
    }
    res.statusCode = 404;
    res.end();
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  issuer = `http://127.0.0.1:${server.address().port}`;
  return {
    env: {
      ENGINE_OIDC_ISSUER: issuer,
      ENGINE_OIDC_CLIENT_ID: "test-client",
      ENGINE_OIDC_CLIENT_SECRET: "test-secret",
      ENGINE_OIDC_ALLOW_HTTP: "true",
    },
    async stop() {
      server.closeAllConnections();
      await new Promise((resolve) => server.close(resolve));
    },
  };
}
export async function loginManagement(url) {
  const verifier = randomBytes(32).toString("base64url");
  const challenge = createHash("sha256").update(verifier).digest("base64url");
  const login = await fetch(`${url}/auth/login`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ challenge }),
  });
  assert.equal(login.status, 200);
  assert.equal(login.headers.get("set-cookie"), null);
  const start = await login.json();
  const authorization = await fetch(start.authorization_url, { redirect: "manual" });
  assert.equal(authorization.status, 302);
  const callback = new URL(authorization.headers.get("location"));
  assert.equal(callback.searchParams.get("state"), start.state);
  const body = { state: start.state, code: callback.searchParams.get("code"), verifier };
  const wrong = await fetch(`${url}/auth/exchange`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ ...body, verifier: randomBytes(32).toString("base64url") }),
  });
  assert.equal(wrong.status, 401);
  const exchange = await fetch(`${url}/auth/exchange`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  assert.equal(exchange.status, 200);
  assert.equal(exchange.headers.get("set-cookie"), null);
  const session = await exchange.json();
  const replay = await fetch(`${url}/auth/exchange`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  assert.equal(replay.status, 401);
  const whoami = await fetch(`${url}/auth/session`, {
    headers: { Authorization: `Bearer ${session.access_token}` },
  });
  assert.equal(whoami.status, 200);
  const cookieOnly = await fetch(`${url}/auth/session`, {
    headers: { Cookie: `tilde_session=${session.access_token}` },
  });
  assert.equal(cookieOnly.status, 401);
  return session.access_token;
}
