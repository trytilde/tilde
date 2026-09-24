---
name: lean-build
description: Build feature work with high overbuilding risk. Use for new behavior, product slices, or integrations where repository reuse, strict scope, and an explicit stop condition matter.
---

# Lean Build

Turn the feature into the narrowest complete outcome that fits the existing
Postgres/cloud architecture.

- Derive observable acceptance and explicit non-goals from the request and repository.
- Trace the entry point through the traits, domain functions, repositories, and routers that own the invariants.
- Deliver one coherent end-to-end path across responsible layers.
- Reuse a fitting seam. Refactor when patching would duplicate behavior, weaken ownership, or hide the root cause.
- Preserve authentication, authorization, tenancy, secrets, provider, database, and ownership-transfer boundaries.
- Omit modes, providers, configuration, extensibility, and polish unless acceptance requires them.
- Add a dependency, service, configuration surface, or migration only when acceptance or lifecycle correctness requires it; state the material tradeoff.

Exercise the path with focused proof. Stop when acceptance passes. Report only
material omissions and the condition that would trigger them.
