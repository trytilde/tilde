---
name: tilde-attachments
description: Read files users send and return files to them through Tilde attachments instead of pasting contents into messages.
---

# Attachments

Conversation messages may reference attachments (images, documents, audio). They are stored
encrypted by Tilde and fetched on demand.

- Read: `await ctx.attachments.download(attachmentId)` returns the bytes; the message's
  attachment entry carries the filename and media type. With the Vercel AI adapter,
  `convertToAiSdkMessages` already inlines supported media for multimodal models.
- Write: `await ctx.attachments.upload({ filename, mediaType, content })` returns an
  attachment to reference from the channel's send tool when it supports files.

Guidelines:

1. Download only what the current reply needs; large files cost time on every wake.
2. Summarise a document's relevant part in the reply rather than echoing it whole.
3. Never forward one user's attachment to a different conversation.
4. When a channel cannot carry files, say what the file contains and offer another route.
