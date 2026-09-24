# ADR: Native Chat and ConnectRPC agent execution

Status: Accepted

Chat is conversation-centric: users and agents participate in rooms, messages
record visible communication, and activity records execution and lifecycle facts.
Inbox, inbox-instance, pipeline and event-bus abstractions are not ported. The
first implementation is native rooms; provider wiring belongs to a later change.
A room must not span multiple channel providers.

Goals/tasks retain explicit agent and room ownership. A run records the durable
objective and an invocation records one reasoning loop. Stopping an invocation
never implies goal/task completion. Dependency edges are fixed at task creation,
remain in one scope, and require existing tasks, preventing cycles without a
generic workflow engine.

Hosted agents implement AgentRuntimeService using ConnectRPC. Invoke is server
streaming; Steer, Cancel and Health are separate methods. SDK return values and
reasoning do not become visible messages. The always-present stop tool terminates
the current loop. Goal/task tools are ordinary dynamic catalog entries, not an
AgentActions trait. Provider tools are not implemented yet.

Visible agent messages use client-streaming SendMessage even for plain strings.
Partial content is persisted and sequenced; completion requires a final frame and
clean EOF. Aborted streams never become completed replies. Room activity cursors
are allocated under a room lock rather than assuming sequence allocation order
is database commit order.

Tilde signs runtime requests using the stored agent signing key and issues random,
hashed-at-rest, invocation-bound callback capabilities. Management authentication
remains deployment-owned. Runtime leases expire; uncertain execution after a crash
is failed for explicit recovery, not automatically replayed. Pending work and
unconsumed steering are durable.

The TypeScript SDK is an independent pnpm workspace at sdk/ts with an example
agent. It follows Dispatch's grouped client and invocation-context pattern but
imports no Dispatch REST client or provider-specific frontend branches.
