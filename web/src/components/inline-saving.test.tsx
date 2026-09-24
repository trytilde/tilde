import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { InlineSaving } from "./inline-saving";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

it("keeps pending saves visible, fades results, and does not let an old timer hide a retry", async () => {
  vi.useFakeTimers();
  const { rerender } = render(<InlineSaving state="saving" label="Settings" resetKey={1} />);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(3000);
  });
  expect(screen.getByRole("status", { name: "Saving Settings" })).toBeTruthy();
  rerender(<InlineSaving state="success" label="Settings" resetKey={1} />);
  expect(screen.getByRole("status", { name: "Settings saved" })).toBeTruthy();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1200);
  });
  rerender(<InlineSaving state="saving" label="Settings" resetKey={2} />);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(800);
  });
  expect(screen.getByRole("status", { name: "Saving Settings" })).toBeTruthy();
  rerender(<InlineSaving state="error" label="Settings" resetKey={2} error="Try again" />);
  expect(screen.getByRole("status", { name: "Settings failed to save" }).title).toBe("Try again");
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1600);
  });
  expect(screen.queryByRole("status", { name: "Settings failed to save" })).toBeNull();
  // An immediately rejected retry may skip a rendered saving state; it still gets fresh feedback.
  rerender(<InlineSaving state="error" label="Settings" resetKey={3} error="Still unavailable" />);
  expect(screen.getByRole("status", { name: "Settings failed to save" }).title).toBe(
    "Still unavailable",
  );
  await act(async () => {
    await vi.advanceTimersByTimeAsync(1600);
  });
  expect(screen.getByRole("status").getAttribute("data-save-state")).toBe("idle");
});
