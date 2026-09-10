Tilde's goal is to be the ubiqutous harness for AI agents. We want to focus on abstracting and managing all the pieces around AI agents and let the agent's themselves be written in any language or framework and support Tilde wrapping the runtime and providing helpful primitives.

# Primitives

- Agent: An external connectRPC server that uses a tilde client SDK to host which we can invoke to trigger an agent
- Agent registry: A registry with all your deployed agents. Used to discover, invoke, audit and health check agents. Almost everything is designed around an agent in the registry, including permissioning, skills, tools, memory, wikis.
- Chat: all of the primitives to manage agent conversations, including sessions (threads, or conversations), identities (user and agent identities, often created by external providers for example a whatsapp phone number of the user you're interacting with), events (things that happen in sessions), providers (integrations with third party chat platforms)
- Connection: A primitive representing managed credentials to a third party API. these will be used for chat providers and in future, as credentials for invoking 

In the future, we plan to broaden the offering to support Tools via MCP, skills registries and more.

# Dos

- Prioritise simple code, don't over abstract or over-use traits.
- prefer clean sql files for sqlx and use the macro to execute over inline or concatanated strings in rust for SQL
- simple light interfaces and traits that can have fat implementations. Traits explain functionality, functionality needs to be simple, trait implementation can be complex
- Keep `docs/` clean. Add documents there only when explicitly asked. Document implementation details and decisions in the relevant source files. If temporary documents are needed to share context with subagents, delete them when that coordination is finished. This overrides automatic documentation or ADR requirements in skills.
- always zeroize and drop secret values decrypted via encryption crate. always encrypt sensitive info via the encryption crate
- Focus on writing tests that test orchestration across boundaries, not unit tests within a single boundary unless there's sufficient complexity. good test = repository -> local postgres, domain interface A -> domain interface B -> domain interface A -> domain interface C.
- Keep a file CONTEXT.md up to date with definitions of domain models and how they interact
- Never assume backwards compatability, always ask first. if not eneded, assume no consumers. drop fields from interfaces and contracts or sql tables
- keep docs folder clean, only add docs to the folder when asked. if required to share conext to subagents, then delete them after. document in code in the relevant files.

# Don'ts

- dont overload or use package.json for script, prefer TaskFile for dev scripts, bash scripts in scripts for more complicated orchestration that taskfile must call
- no extra fields on structs or overengineered structs that plan for future functionality
- no bags of JSON blob or "metadata" fields that we stuff interface implementation into isntead of structured fields unless explicitly approved
- no overuse of traits
- no overuse of generics & dynamic trait implementations
- no slop tests designed for coverage increase or simple checks or things with zero complexity (like testing a field setter) bad test = assert agent.created_at = today
- Use shadcn primitives and React Hook Form for React forms. Provider setup helpers belong in `sdk/ts/packages/connection-ui`; keep them small. Static credentials and standard OAuth use shared forms; only custom setup flows have provider-owned `ui.tsx` files. Provider HTML is generated from the shared template, not copied into each catalog folder.
