import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useCursorPage, type FetchPage } from "./use-cursor-page";

afterEach(cleanup);
const pages: Record<string, { items: string[]; nextPageToken: string }> = {
  "": { items: ["Ada", "Grace"], nextPageToken: "opaque-cursor-B" },
  "opaque-cursor-B": {
    items: ["Linus", "Margaret"],
    nextPageToken: "opaque-cursor-C",
  },
  "opaque-cursor-C": { items: ["Alan"], nextPageToken: "" },
};

describe("server cursor pagination", () => {
  it("uses returned cursors, retains previous tokens, and stops at the last page", async () => {
    const fetchPage = vi.fn<FetchPage<string>>(async ({ pageToken }) => pages[pageToken]);
    const { result } = renderHook(() => useCursorPage(fetchPage, 2));
    await waitFor(() => expect(result.current.items).toEqual(["Ada", "Grace"]));
    expect(result.current.history).toEqual([]);
    act(() => {
      result.current.previous();
    });
    expect(fetchPage).toHaveBeenCalledTimes(1);
    act(() => {
      result.current.next();
    });
    await waitFor(() => expect(result.current.index).toBe(1));
    expect(fetchPage.mock.calls[1][0]).toEqual({
      pageToken: "opaque-cursor-B",
      pageSize: 2,
    });
    act(() => {
      result.current.next();
    });
    await waitFor(() => expect(result.current.items).toEqual(["Alan"]));
    expect(result.current.nextToken).toBe("");
    act(() => {
      result.current.next();
    });
    expect(fetchPage).toHaveBeenCalledTimes(3);
    act(() => {
      result.current.previous();
    });
    await waitFor(() => expect(result.current.items).toEqual(["Linus", "Margaret"]));
    act(() => {
      result.current.previous();
    });
    await waitFor(() => expect(result.current.index).toBe(0));
    expect(fetchPage.mock.lastCall![0].pageToken).toBe("");
  });

  it("restarts with an empty cursor when the page size changes", async () => {
    const fetchPage = vi.fn<FetchPage<string>>(async ({ pageToken }) => pages[pageToken]);
    const { result } = renderHook(() => useCursorPage(fetchPage, 2));
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => {
      result.current.next();
    });
    await waitFor(() => expect(result.current.index).toBe(1));
    act(() => {
      result.current.resize(50);
    });
    await waitFor(() => expect(result.current.size).toBe(50));
    expect(fetchPage.mock.lastCall![0]).toEqual({
      pageToken: "",
      pageSize: 50,
    });
    expect(result.current.history).toEqual([]);
    expect(result.current.index).toBe(0);
  });

  it("keeps the displayed page and cursor history on failure, and retries the failed request", async () => {
    const fetchPage = vi.fn<FetchPage<string>>(async ({ pageToken }) => pages[pageToken]);
    const { result } = renderHook(() => useCursorPage(fetchPage, 2));
    await waitFor(() => expect(result.current.loading).toBe(false));
    fetchPage.mockRejectedValueOnce(new Error("Cursor expired"));
    act(() => {
      result.current.next();
    });
    await waitFor(() => expect(result.current.error).toBe("Cursor expired"));
    expect(result.current.index).toBe(0);
    expect(result.current.items).toEqual(["Ada", "Grace"]);
    expect(result.current.history).toEqual([]);
    act(() => {
      result.current.retry();
    });
    await waitFor(() => expect(result.current.index).toBe(1));
    expect(result.current.error).toBe("");
    expect(fetchPage.mock.lastCall![0].pageToken).toBe("opaque-cursor-B");
  });

  it("ignores stale responses when a new request supersedes an old one", async () => {
    let finishOld!: (value: { items: string[]; nextPageToken: string }) => void;
    const fetchPage = vi
      .fn<FetchPage<string>>()
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finishOld = resolve;
          }),
      )
      .mockResolvedValueOnce({ items: ["Current"], nextPageToken: "" });
    const { result } = renderHook(() => useCursorPage(fetchPage));
    act(() => {
      result.current.resize(20);
    });
    await waitFor(() => expect(result.current.items).toEqual(["Current"]));
    expect(fetchPage.mock.calls[0][1].aborted).toBe(true);
    await act(async () => {
      finishOld({ items: ["Stale"], nextPageToken: "obsolete" });
    });
    expect(result.current.items).toEqual(["Current"]);
    expect(result.current.nextToken).toBe("");
    expect(result.current.size).toBe(20);
  });
});
