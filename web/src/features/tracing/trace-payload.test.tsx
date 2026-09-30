import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { TracePayload } from "./trace-payload";

const rpc = vi.hoisted(() => ({ getTraceObjectUrl: vi.fn() }));
vi.mock("@/client", () => ({ traces: rpc }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
const id = "b".repeat(64);
const token = `@@@tildeMedia:type=image/png|id=${id}|source=base64_data_uri@@@`;

it("keeps the owning agent for media inside conversation payloads", async () => {
  rpc.getTraceObjectUrl.mockResolvedValue({ url: "https://objects.test/one", expiresIn: 600 });
  const payload = {
    messages: [
      { role: "user", content: [{ type: "text", text: `first ${token}` }] },
      { role: "assistant", content: `second ${token}` },
    ],
  };
  render(<TracePayload text={JSON.stringify(payload)} agentId="agent-two" />);
  const images = await screen.findAllByRole("img");
  expect(images).toHaveLength(2);
  await waitFor(() => expect(images[1].getAttribute("src")).toBe("https://objects.test/one"));
  expect(rpc.getTraceObjectUrl).toHaveBeenCalledTimes(2);
  for (const call of rpc.getTraceObjectUrl.mock.calls)
    expect(call[0]).toEqual({ agentId: "agent-two", key: `media/agent-two/${id}` });
  expect(screen.queryByText(/@@@tildeMedia/)).toBeNull();
});
