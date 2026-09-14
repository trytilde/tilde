# Langfuse UI attribution

The Formatted/JSON view toggle and duration/cost formatting in this directory are
adapted from Langfuse at `185ad9b0988d403bd0ab7b16bb0bf7a6febe1101`:

- `web/src/features/traces/components/IOPreview/components/ViewModeToggle.tsx`
- `web/src/utils/numbers.ts`

MIT license: see LANGFUSE-LICENSE. Tilde uses its own shadcn controls, routing and
management API. No Langfuse authentication, tRPC, analytics or EE code is included.
The observation grid and tree/detail composition follow the Langfuse interaction
model; they are implemented for Tilde's typed API and TanStack Table version.
