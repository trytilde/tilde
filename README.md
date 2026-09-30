<p align="center">
  <img alt="Tilde" src=".github/assets/tilde-banner.png" width="100%">
</p>
<p align="center">
  <a href="https://github.com/trytilde/tilde/graphs/contributors"><img alt="GitHub contributors" src="https://img.shields.io/github/contributors/trytilde/tilde"/></a>
  <a href="http://makeapullrequest.com"><img alt="PRs welcome" src="https://img.shields.io/badge/PRs-welcome-brightgreen.svg?style=shields"/></a>
  <a href="https://github.com/trytilde/tilde/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/github/license/trytilde/tilde"/></a>
  <a href="https://github.com/trytilde/tilde/commits/main"><img alt="GitHub commit activity" src="https://img.shields.io/github/commit-activity/m/trytilde/tilde"/></a>
  <a href="https://github.com/trytilde/tilde/issues?q=is%3Aissue%20state%3Aclosed"><img alt="GitHub closed issues" src="https://img.shields.io/github/issues-closed/trytilde/tilde"/></a>
</p>

<p align="center">
  <a href="https://trytilde.ai/docs">Docs</a> - <a href="https://trytilde.ai/waitlist?utm_source=github&utm_medium=readme&utm_campaign=tilde_oss">Tilde Cloud waitlist</a> - <a href="https://discord.gg/a9eUdFZca">Community</a> - <a href="https://trytilde.ai/why-tilde">Why Tilde?</a> - <a href="https://github.com/trytilde/tilde/blob/main/CHANGELOG.md">Changelog</a> - <a href="https://github.com/trytilde/tilde/issues/new">Bug reports</a>
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
  - [Tilde Cloud (recommended)](#tilde-cloud-recommended)
  - [Self-hosting Tilde (advanced)](#self-hosting-tilde-advanced)
- [Setting up Tilde](#setting-up-tilde)
- [Learning more about Tilde](#learning-more-about-tilde)
- [Contributing](#contributing)
- [Open source vs. paid](#open-source-vs-paid)

## Getting started with Tilde

### Tilde Cloud (recommended)

The fastest and most reliable way to get started will be [Tilde Cloud](https://trytilde.ai/waitlist?utm_source=github&utm_medium=readme&utm_campaign=tilde_oss). We run the gateway, its databases, and upgrades, so you only deploy your agents. [Join the waitlist](https://trytilde.ai/waitlist?utm_source=github&utm_medium=readme&utm_campaign=tilde_oss) to get access.

### Self-hosting Tilde (advanced)

The gateway is one container image, `ghcr.io/trytilde/tilde`. It needs PostgreSQL, ClickHouse, and S3-compatible storage. [`compose.yaml`](compose.yaml) starts those dependencies and is a good starting point for a single machine. See [Deploy the gateway](https://trytilde.ai/docs/deployment/gateway) for configuration and network exposure.

> [!WARNING]
> Open-source Tilde has no operator sign-in. The web UI and management API accept every request that reaches them. You must put a reverse proxy or another authentication method in front of them before anyone else can reach the gateway.

We _do not_ provide customer support or offer guarantees for self-hosted deployments. For production, we recommend [Tilde Cloud or Tilde Enterprise](https://trytilde.ai/docs/editions).

## Setting up Tilde

Once you have a gateway, register an agent in the UI and connect it with one of our SDKs. Your agent opens an outbound connection to the gateway and never listens for requests. We have SDKs and adapters for popular agent frameworks:

| TypeScript ([`@trytilde/sdk`](sdk/ts)) | Python ([`trytilde`](sdk/py)) |
| --- | --- |
| [Vercel AI SDK](https://trytilde.ai/docs/frameworks/vercel-ai-sdk) | [LangChain](https://trytilde.ai/docs/frameworks/langchain) |
| [Mastra](https://trytilde.ai/docs/frameworks/mastra) | [Pydantic AI](https://trytilde.ai/docs/frameworks/pydantic-ai) |
| [LangChain](https://trytilde.ai/docs/frameworks/langchain) | [OpenAI Agents](https://trytilde.ai/docs/frameworks/openai-agents) |
| [OpenAI Agents](https://trytilde.ai/docs/frameworks/openai-agents) | [Agno](https://trytilde.ai/docs/frameworks/agno) |
| | [CrewAI](https://trytilde.ai/docs/frameworks/crewai) |

Agents in other languages or frameworks use the core SDKs directly. See [Anatomy of an agent](https://trytilde.ai/docs/anatomy-of-an-agent), then [release agents from CI](https://trytilde.ai/docs/deployment/ci-cd).

## Learning more about Tilde

Read the [documentation](https://trytilde.ai/docs) for every feature, and [why Tilde exists](https://trytilde.ai/why-tilde). For how this repository fits together, see [architecture](docs/architecture.md), [code structure](docs/code-structure.md), and [patterns](docs/patterns.md).

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
