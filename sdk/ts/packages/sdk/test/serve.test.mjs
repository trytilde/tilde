import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http2";
import { once } from "node:events";
import { connectNodeAdapter } from "@connectrpc/connect-node";
import { RunService, serve } from "../dist/index.js";

const run = async () => {};
function setEnv(values) {
  for (const [k, v] of Object.entries(values)) {
    if (v === undefined) delete process.env[k];
    else process.env[k] = v;
  }
}
function withEnv(values, body) {
  const saved = Object.fromEntries(Object.keys(values).map((k) => [k, process.env[k]]));
  setEnv(values);
  return Promise.resolve(body()).finally(() => setEnv(saved));
}
const plain = {
  TILDE_DISCOVERY: undefined,
  AWS_LAMBDA_FUNCTION_NAME: undefined,
  TILDE_DEPLOYMENT_TOKEN: undefined,
  TILDE_GATEWAY_URL: undefined,
};

void test("a plain import and tilde deploy's discovery start nothing", async () => {
  for (const env of [plain, { ...plain, TILDE_DISCOVERY: "1", TILDE_DEPLOYMENT_TOKEN: "t" }])
    await withEnv(env, async () => {
      const served = serve({ run });
      assert.equal(served.agent, undefined);
      await assert.rejects(served({}), /only runs in AWS Lambda/);
    });
});

void test("inside Lambda serve is the Lambda handler", async () => {
  await withEnv({ ...plain, AWS_LAMBDA_FUNCTION_NAME: "agent" }, async () => {
    const served = serve({ run });
    assert.equal(served.agent, undefined);
    // The handler reads the event as an InvokeRequest rather than refusing to run.
    await assert.rejects(
      served({ notAField: 1 }),
      (error) => !/only runs in AWS Lambda/.test(error.message),
    );
  });
});

void test("with a deployment token serve dials in", { timeout: 10000 }, async () => {
  const watched = [];
  const gateway = createServer(
    connectNodeAdapter({
      routes(router) {
        router.service(RunService, {
          async *watch(request, ctx) {
            watched.push(ctx.requestHeader.get("authorization"));
            yield { frame: { case: "registered", value: { instanceId: request.instanceId } } };
            await new Promise((resolve) => ctx.signal.addEventListener("abort", resolve));
          },
          async heartbeat() {
            return {};
          },
        });
      },
    }),
  );
  gateway.listen(0, "127.0.0.1");
  await once(gateway, "listening");
  await withEnv(
    {
      ...plain,
      TILDE_GATEWAY_URL: `http://127.0.0.1:${gateway.address().port}`,
      TILDE_DEPLOYMENT_TOKEN: "deployment-secret",
    },
    async () => {
      const served = serve({ run });
      assert.ok(served.agent);
      for (let i = 0; i < 100 && !served.agent.registration; i++)
        await new Promise((resolve) => setTimeout(resolve, 20));
      assert.ok(served.agent.registration, "registered with the gateway");
      assert.deepEqual(watched, ["Bearer deployment-secret"]);
      await served.agent.close();
    },
  );
  gateway.close();
});
