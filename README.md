<p align="center">
  <img alt="Tilde" src=".github/assets/tilde-banner.png" width="100%">
</p>
<p align="center">
  <a href="http://makeapullrequest.com"><img alt="PRs welcome" src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=shields"/></a>
  <a href="https://github.com/trytilde/tilde/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/github/license/trytilde/tilde"/></a>
</p>

<p align="center">
  <a href="https://trytilde.ai/docs">Docs</a> - <a href="https://trytilde.ai/waitlist?utm_source=github&utm_medium=readme&utm_campaign=tilde_oss">Tilde Cloud waitlist</a> - <a href="https://discord.gg/a9eUdFZca">Community</a> - <a href="https://github.com/trytilde/tilde/blob/main/CHANGELOG.md">Changelog</a> - <a href="https://github.com/trytilde/tilde/issues/new">Bug reports</a>
</p>

## Tilde is the open-source agent registry for AI-native enterprises

[Tilde](https://trytilde.ai) gives your agents a home. Register every agent your company builds, in any language or framework, and govern, deploy, monitor and manage them from one place. Your agent connects to the Tilde gateway through our SDKs, and Tilde handles the rest:

- [Agent registry](https://trytilde.ai/docs/agent-registry): Register agents built in any framework, deploy them, check their health, and pause or retire them from one place.
- [Chat channels](https://trytilde.ai/docs/chat-channels): Route messages from Slack, WhatsApp, email, SMS and iMessage, GitHub, and your own apps to agents, with sessions, identities, and access control.
- [Connections](https://trytilde.ai/docs/connections): Store the credentials agents use to reach chat, tool, and inference providers, encrypted with a key you control. Agents never see them.
- [Tools](https://trytilde.ai/docs/tools/overview): Give agents managed provider tools, MCP servers, your own tool servers, and tools bundled in their code, all audited by the gateway.
- [Inference gateway](https://trytilde.ai/docs/inference-providers): Send every LLM request through Tilde to keep provider keys out of agents, track usage and cost, and enforce budgets.
- [Prompts](https://trytilde.ai/docs/prompts/overview): Version the prompts in your agent's code and see which version produced each model call, with cost and latency per version.
- [Skills](https://trytilde.ai/docs/skills/overview): Give agents reusable instructions and files from your code, from Git, or from the Tilde catalog.
- [Routing](https://trytilde.ai/docs/routing): Choose which deployment receives new conversations, with latest or weighted routing, conversation pinning, and sidecar failover.
- [Sessions](https://trytilde.ai/docs/sessions) and [traces](https://trytilde.ai/docs/traces): Review every conversation, model call, and tool call, with logs, token usage, cost, and errors, stored in Tilde and forwardable over OTLP.

[Join the Tilde Cloud waitlist](https://trytilde.ai/waitlist?utm_source=github&utm_medium=readme&utm_campaign=tilde_oss), or run it yourself from this repository.

## Table of contents

- [Tilde is the open-source agent registry for AI-native enterprises](#tilde-is-the-open-source-agent-registry-for-ai-native-enterprises)
- [Table of contents](#table-of-contents)
- [Getting started with Tilde](#getting-started-with-tilde)
  - [Deploying](#deploying)
- [Setting up Tilde](#setting-up-tilde)
- [Learning more about Tilde](#learning-more-about-tilde)
- [Contributing](#contributing)
- [Open source vs. paid](#open-source-vs-paid)

## Getting started with Tilde

Three steps: run Tilde, install the CLI, start your agent.

**1. Run Tilde.** The gateway is one container image, `ghcr.io/trytilde/tilde`, which needs PostgreSQL, ClickHouse and S3-compatible storage. [`quickstart/compose.yaml`](quickstart/compose.yaml) runs the latest image with all three on your machine:

```bash
git clone https://github.com/trytilde/tilde.git && cd tilde/quickstart
echo "TILDE_ENCRYPTION_KEY=$(openssl rand -base64 32)" > .env
docker compose up -d --wait
```

Open [http://127.0.0.1:8080](http://127.0.0.1:8080) and register your first agent, giving it the capabilities, inference and access it needs. Keep `.env`: the key encrypts the credentials Tilde stores. Set `TILDE_VERSION` in `.env` to pin a release instead of `latest`.

**2. Install the CLI.**

```bash
curl -fsSL https://trytilde.ai/install.sh | sh
```

It is also on [npm](https://www.npmjs.com/package/@trytilde/cli) and [PyPI](https://pypi.org/project/trytilde-cli/), and the SDKs ask for the matching version, so `npm install @trytilde/sdk` or `uv add trytilde` puts the same `tilde` in your project. Run it there with `npx tilde` or `uv run tilde` to keep the CLI and the SDK on one version.

**3. Start your agent.** In an agent project, with the gateway running:

```bash
tilde dev
```

`tilde dev` matches the registered agent with your project's name (`--name` or `--agent-id` to pick another), registers a deployment carrying the prompts, skills and tools your code declares, starts your agent with the credentials it needs, and serves a chat page at [http://127.0.0.1:4242](http://127.0.0.1:4242) to talk to it. Edit a prompt or a skill and save: it registers the next deployment and restarts the agent.

It never creates the agent for you. An agent that exists without its capabilities, inference and access cannot serve an invocation, so registering one stays a deliberate step in the UI. If you have no agent code yet, start from [`sdk/ts/examples`](sdk/ts/examples) or [`sdk/py/examples`](sdk/py/examples), or read [Anatomy of an agent](https://trytilde.ai/docs/anatomy-of-an-agent).

`tilde doctor` explains what is wrong when any of that does not work, and `tilde deploy` is the same registration from CI. Run `tilde --help` for everything else.

> [!WARNING]
> Open-source Tilde has no operator sign-in. The web UI and management API accept every request that reaches them. The quickstart binds every port to `127.0.0.1`; put a reverse proxy or another authentication method in front of them before anyone else can reach the gateway.

### Deploying

The easiest way to run Tilde in production is [Tilde Cloud](https://trytilde.ai/waitlist?utm_source=github&utm_medium=readme&utm_campaign=tilde_oss): we run the gateway, its databases and upgrades, so you only deploy your agents. [Join the waitlist](https://trytilde.ai/waitlist?utm_source=github&utm_medium=readme&utm_campaign=tilde_oss) for access, or [book a call](https://trytilde.ai/enterprise?utm_source=github&utm_medium=readme&utm_campaign=tilde_oss) if you are an enterprise.

To run it yourself, compare [Tilde editions](https://trytilde.ai/docs/editions) and read the [deployment docs](https://trytilde.ai/docs/deployment/gateway).

## Setting up Tilde

Your agent connects with one of our SDKs, opening an outbound connection to the gateway; it never listens for requests. `tilde dev` registers it for you, and the UI can do it by hand. We have SDKs and adapters for popular agent frameworks:

| TypeScript ([`@trytilde/sdk`](sdk/ts)) | Python ([`trytilde`](sdk/py)) |
| --- | --- |
| [Vercel AI SDK](https://trytilde.ai/docs/frameworks/vercel-ai-sdk) | [LangChain](https://trytilde.ai/docs/frameworks/langchain) |
| [Mastra](https://trytilde.ai/docs/frameworks/mastra) | [Pydantic AI](https://trytilde.ai/docs/frameworks/pydantic-ai) |
| [LangChain](https://trytilde.ai/docs/frameworks/langchain) | [OpenAI Agents](https://trytilde.ai/docs/frameworks/openai-agents) |
| [OpenAI Agents](https://trytilde.ai/docs/frameworks/openai-agents) | [Agno](https://trytilde.ai/docs/frameworks/agno) |
| | [CrewAI](https://trytilde.ai/docs/frameworks/crewai) |

Agents in other languages or frameworks use the core SDKs directly. See [Anatomy of an agent](https://trytilde.ai/docs/anatomy-of-an-agent), then [release agents from CI](https://trytilde.ai/docs/deployment/ci-cd).

## Learning more about Tilde

Read the [documentation](https://trytilde.ai/docs) for every feature. For how this repository fits together, see [architecture](docs/architecture.md), [code structure](docs/code-structure.md), and [patterns](docs/patterns.md).

## Contributing

We <3 contributions big and small:

- Join the [community](https://discord.gg/a9eUdFZca) to ask questions and share what you are building.
- Open a PR. Most changes here are written by coding agents, which follow [`AGENTS.md`](AGENTS.md), [`CONTEXT.md`](CONTEXT.md), and the [patterns](docs/patterns.md). Add a Changie fragment (`task change`) to every PR.
- Submit a feature request or a [bug report](https://github.com/trytilde/tilde/issues/new).

To develop locally, install [Task](https://taskfile.dev), Rust, Node.js with pnpm, uv and Docker, then:

```bash
task setup   # JavaScript dependencies and pinned code generators
task dev     # Gateway, web UI and local dependencies (copy .env.example to .env first)
task test    # Rust, SDK and web tests against disposable Postgres
task check   # Generated code, formatting, lint and Clippy
```

## Open source vs. paid

This repository is available under the [Apache 2.0 license](LICENSE).

[Tilde Cloud and Tilde Enterprise](https://trytilde.ai/docs/editions) add operator sign-in through your identity provider, roles and groups, management API keys, and, in Tilde Cloud, Tilde's managed OAuth apps for tools. Tilde Enterprise runs in your own infrastructure with our support.
