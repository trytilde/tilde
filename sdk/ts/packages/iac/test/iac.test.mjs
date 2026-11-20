import test from "node:test";
import assert from "node:assert/strict";
import { fromJson } from "@bufbuild/protobuf";
import {
  AgentSpecSchema,
  GroupSpecSchema,
  UserSpecSchema,
} from "@trytilde/contracts/tilde/management/v1/apply_pb.js";
import {
  ALL,
  SPEC,
  blueprint,
  connection,
  defineAgent,
  defineProject,
  egress,
  github,
  lambda,
  openai,
  routine,
  secret,
  defineTeam,
  grant,
  group,
  user,
} from "../dist/index.js";

const linear = connection.ref("linear/acme", { type: "api_key" });

test("an agent spec is AgentSpec protobuf JSON the server accepts", () => {
  const agent = defineAgent({
    key: "triage",
    name: "Triage",
    capabilities: { agentsRead: { mode: "TARGET_SELECTION_ALL" } },
    concurrency: "queue_and_batch",
    toolMode: "dynamic",
    inference: { default: openai.ref("prod"), old: { delete: true } },
    tools: [
      github.ref("acme").tools("github_get_issue").untypedTools("new_tool").tools(ALL),
      linear.untypedTools("x").tools("get_issue"),
      connection.ref("sentry/acme").tools({ delete: true }),
    ],
    sandbox: blueprint({
      name: "box",
      provider: connection.ref("e2b/acme"),
      template: "tilde-base",
      env: { MODE: "strict", NPM_TOKEN: secret.ref("NPM_TOKEN"), OLD: { delete: true } },
      egress: [egress("github", github.ref("acme"), { pull_requests: true })],
    }),
    routines: [
      routine({ name: "Daily", prompt: "Sweep", cron: "0 9 * * *" }),
      routine({
        name: "On issue",
        prompt: "Look",
        signal: {
          connection: linear,
          type: "issue.created",
          filters: { "issue.team.key": ["ENG"] },
        },
        enabled: true,
      }),
      routine({ name: "Gone", prompt: "", delete: true }),
    ],
    deploy: lambda("arn:aws:lambda:us-east-1:1:function:triage:3"),
  });
  assert.equal(agent[SPEC], "agent");
  // The server's own contract parses it: field names and enum values are right.
  const parsed = fromJson(AgentSpecSchema, agent.json);
  assert.equal(parsed.key, "triage");
  assert.deepEqual(
    parsed.tools.map((t) => [t.connection.slug, t.tools, t.delete]),
    [
      ["github/acme", ["github_get_issue", "new_tool", "*"], false],
      ["linear/acme", ["x", "get_issue"], false],
      ["sentry/acme", [], true],
    ],
  );
  assert.equal(parsed.tools[1].connection.type, "api_key");
  assert.deepEqual(
    parsed.inference.map((a) => [a.alias, a.connection?.slug, a.delete]),
    [
      ["default", "openai/prod", false],
      ["old", undefined, true],
    ],
  );
  assert.equal(parsed.sandbox.env.MODE.value.value, "strict");
  assert.equal(parsed.sandbox.env.OLD.delete, true);
  assert.equal(parsed.sandbox.env.NPM_TOKEN.value.case, "secret");
  assert.equal(parsed.sandbox.env.NPM_TOKEN.value.value.name, "NPM_TOKEN");
  assert.equal(parsed.sandbox.egress[0].enabled, true);
  assert.deepEqual(JSON.parse(parsed.sandbox.egress[0].configJson), { pull_requests: true });
  assert.equal(parsed.routines[0].enabled, undefined, "absent enabled is left to people");
  assert.equal(parsed.routines[1].trigger.kind.value.filters[0].values[0], "ENG");
  assert.equal(parsed.routines[2].delete, true);
  assert.equal(parsed.deployment.targetReference, "arn:aws:lambda:us-east-1:1:function:triage:3");
});

test("ALL is the star and both selection methods accumulate in any order", () => {
  assert.equal(ALL, "*");
  const one = github.ref("acme").untypedTools("a").tools("github_get_issue");
  const two = github.ref("acme").tools("github_get_issue").untypedTools("a");
  assert.deepEqual([...one.names].sort(), [...two.names].sort());
  assert.equal(one.connection.slug, "github/acme");
});

test("refs and routines refuse what the server would", () => {
  assert.throws(() => connection.ref("acme"), /provider\/name/);
  assert.throws(() => secret.ref("npm-token"), /not a valid name/);
  assert.throws(
    () => defineAgent({ key: "a", name: "A", routines: [routine({ name: "R", prompt: "P" })] }),
    /exactly one of cron and signal/,
  );
});

test("people: a team's users and groups, and an agent's own with its grants", () => {
  const team = defineTeam({
    team: "faro",
    users: [
      user({ email: "ada@faro.test", name: "Ada", login: { role: "admin" } }),
      user({ email: "old@faro.test", delete: true }),
    ],
    groups: [
      group({ name: "eng", members: ["ada@faro.test", { email: "old@faro.test", delete: true }] }),
    ],
  });
  assert.equal(team[SPEC], "team");
  assert.equal(team.team, "faro");
  const [ada, old] = team.users.map((u) => fromJson(UserSpecSchema, u));
  assert.equal(ada.team.role, 2, "login: { role } sets the team role");
  assert.deepEqual(
    [ada.email, ada.name, ada.login, old.delete],
    ["ada@faro.test", "Ada", true, true],
  );
  const eng = fromJson(GroupSpecSchema, team.groups[0]);
  assert.deepEqual(
    eng.members.map((m) => [m.email, m.delete]),
    [
      ["ada@faro.test", false],
      ["old@faro.test", true],
    ],
  );
  const agent = fromJson(
    AgentSpecSchema,
    defineAgent({
      key: "triage",
      name: "Triage",
      users: [
        user({ email: "bob@faro.test", name: "Bob", login: true }),
        user({ email: "eve@faro.test", team: { delete: true } }),
      ],
      access: [
        grant(group("eng"), "ownership"),
        grant(user("ada@faro.test"), "visibility", { delete: true }),
      ],
    }).json,
  );
  assert.equal(agent.users[0].email, "bob@faro.test");
  assert.equal(agent.users[0].team.role, 1, "login: true is a member");
  assert.equal(agent.users[1].team.delete, true);
  assert.deepEqual(
    agent.access.map((g) => [g.principal.case, g.principal.value, g.plane, g.delete]),
    [
      ["group", "eng", 2, false],
      ["user", "ada@faro.test", 1, true],
    ],
  );
  assert.throws(() => user("not-an-email"), /not an email/);
});

test("a project carries its team and agents", () => {
  const a = defineAgent({ key: "a", name: "A" });
  const project = defineProject({ team: "faro", agents: [a] });
  assert.equal(project[SPEC], "project");
  assert.equal(project.team, "faro");
  assert.equal(project.agents[0], a);
});
