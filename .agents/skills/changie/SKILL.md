---
name: changie
description: Add or verify a Changie changeset for every PR, including blank acknowledgement changesets for PRs with no release-note-worthy change.
metadata:
  author: tilde
  version: "1.0.0"
---

# Changie Changesets

## Repository documentation requirements

Follow [docs/README.md](../../../docs/README.md) for every change in this workflow. Create or update
ADRs for resolved durable decisions, keep affected README/setup/public docs current,
and maintain the complete pending or PR-numbered update record after every revision.
Use the shared templates and section names. Missing or stale required documentation
blocks completion. Document already authorized decisions without asking again; ask
only about unresolved choices. These requirements govern documentation instructions
elsewhere in this skill; preserve its repository-specific implementation and checks.

Use this whenever preparing, reviewing, or opening a PR, or whenever the user
mentions Changie, changesets, changelog entries, release notes, or blank
changesets.

Every PR must add one new file under `.changes/unreleased/`. A PR with no
release-note-worthy change still needs a blank acknowledgement changeset.

## Standard Entry

Prefer the Changie CLI for normal entries:

```bash
changie new --kind Changed --body "Describe the user-visible or operator-visible change." --interactive=false
```

Choose `Added`, `Changed`, or `Removed` from `.changie.yaml`.

## Blank Acknowledgement

Use a blank acknowledgement only when the PR truly should not add a release note,
such as test-only maintenance, internal agent documentation, or CI-only changes.
The Changie CLI rejects an empty body, so create the YAML file manually:

```yaml
kind: Changed
body: ""
time: 2026-07-07T15:06:04+02:00
```

Name the file like `.changes/unreleased/Changed-YYYYMMDD-HHMMSS.yaml`. Use the
current local timestamp from:

```bash
date --iso-8601=seconds
```

## Verification

Before handing off or opening a PR, run:

```bash
task check-changie
task version:check
```

The check requires a newly added `.changes/unreleased/*.yaml` file in the PR
diff and validates that it has a configured `kind:` and a `body:` field. A blank
acknowledgement must use `body: ""`.
