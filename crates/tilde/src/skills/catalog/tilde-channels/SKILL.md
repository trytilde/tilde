---
name: tilde-channels
description: Reply on the chat channel the conversation arrived from using the provider tools Tilde exposes, rather than returning model text.
---

# Replying through Tilde channels

Model output is private: nothing the user sees is produced by returning text. Every visible
reply is a tool call on the conversation's channel.

1. Load the history with `ctx.message.history()`; the latest conversation message is what
   you answer. Objective, goal and task entries in the history are context, not messages.
2. Use the tools in `ctx.channel.current` (or `ctx.tools` with a framework adapter such as
   `convertToAiSdkTools`). Each tool's description names the provider it belongs to and the
   format it accepts (plain text, Markdown, a template) and whether it supports replies,
   reactions or typing indicators.
3. Send one reply per turn unless the channel's tool documents threading. Prefer the reply
   tool that references the message you are answering when one exists.
4. Call `ctx.setTyping(true)` before slow work and `ctx.setTyping(false)` after sending.
5. If the channel offers no send tool (for example a read-only route), record the result in a
   task instead of failing.

Do not paste the model's reasoning into the channel, and never include credentials, tool
schemas or internal identifiers in a visible reply.
