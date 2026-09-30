---
name: tilde-goals-and-tasks
description: Track multi-step work with Tilde goals and tasks so progress survives interruptions, suspensions and hand-offs between invocations.
---

# Goals and tasks

Goals and tasks are owned by the agent within the current thread and are hydrated into every
later invocation, so they are the right place for state that must outlive one wake.

- `ctx.goals.create({ objective })` opens a goal; `ctx.goals.update({ id, status })` closes it
  with `completed`, `failed` or `canceled`.
- `ctx.tasks.create({ title, goalId, dependencyIds })` adds a step; move it through
  `working`, `blocked` (with `blockedReason`), `completed` or `failed` with `ctx.tasks.update`.
- `ctx.goals.list()` and `ctx.tasks.list()` show what earlier invocations left behind; read
  them before starting new work so a resumed run continues rather than restarts.

Guidelines:

1. Create a goal only for work that spans more than one reply or one invocation.
2. Keep task titles imperative and specific ("Confirm delivery address"), one outcome each.
3. Mark a task `blocked` with a reason when you need something from the user, then ask for it
   on the channel; the next invocation reads the reason and knows what to check.
4. When a suspend command arrives, update tasks before the checkpoint so the state is exact.
5. Close the goal when the objective is met; leave nothing `working` at the end of a run.
