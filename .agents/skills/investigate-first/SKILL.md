---
name: investigate-first
description: Diagnose ambiguous failures before editing. Use for unknown causes, intermittent behavior, performance regressions, or investigations needing evidence-ranked hypotheses.
---

# Investigate First

Gather evidence before changing product code.

- Separate the observed symptom from the inferred cause.
- Trace inputs, state transitions, tenant boundaries, ownership, and failure output.
- Rank hypotheses by evidence and cheap falsification value.
- Do not edit until one credible mechanism explains the evidence.
- Stop exploration when evidence is sufficient to name the cause or exact blocker.

Report the cause and proof. Make no fix unless the task authorizes implementation.
