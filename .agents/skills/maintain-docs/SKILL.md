---
name: maintain-docs
description: Maintain repository ADRs, change and PR update records, and affected setup/public docs while implementing or reviewing changes.
---

# Maintain repository documentation

- Follow [docs/README.md](../../../docs/README.md)
- Read governing ADRs before implementation. 
- Write accepted durable decisions in `docs/adrs/`; amend current text and append timestamped history when they change.
- Record already authorized decisions directly and ask only about unresolved choices.
- Write one update record using [the template](../../../docs/updates/template.md). Before a PR exists, use `docs/updates/pending/<short-slug>.md`. Once open, rename it to `docs/updates/<actual-pr-number>.md`.
- In each domain in src folder, always add a README.md explaining the  modelling, public interfaces exposed and general use / purpose. include explicit Dos and Don'ts where necessary. Aim is for constumers to avoid footguns.
- Review affected READMEs, setup/deployment instructions, generated contracts, and
public documentation. Update them in the same change,