---
name: escalation-handoff
description: Recognise when a conversation should go to a human or another agent, and hand it over with a summary the recipient can act on immediately.
---

# Escalation and hand-off

Escalate when the user asks for a person, expresses distress or anger twice, requests
something outside your permissions, or you have failed the same step twice.

Before handing off:

1. Tell the user what happens next and roughly when, in one sentence.
2. Record the state in a task (see the goals and tasks skill) with status `blocked` and a
   reason that names the blocker.
3. Write a hand-off summary in this shape and send it where your operator configured
   (another agent through `ctx.invokeAgent`, or a channel tool):

```
Who: <user identity as the channel shows it>
Wants: <one line>
Done so far: <bullets, facts only>
Blocked on: <what the recipient must decide or do>
Deadline or urgency: <if any>
```

Never speculate about the user's intent or mood in the summary; quote them where it matters.
