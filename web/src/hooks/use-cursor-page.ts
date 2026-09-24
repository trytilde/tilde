import { useCallback, useEffect, useRef, useState } from "react";

type Position = {
  token: string;
  index: number;
  size: number;
  history: string[];
};
type Page<T> = Position & { items: T[]; nextToken: string; loaded: boolean };
export type FetchPage<T> = (
  request: { pageToken: string; pageSize: number },
  signal: AbortSignal,
) => Promise<{ items: T[]; nextPageToken: string }>;

/** Server cursors are opaque. Previous reuses visited cursors; no total-page count is inferred. */
export function useCursorPage<T>(fetchPage: FetchPage<T>, initialSize = 10) {
  const [page, setPage] = useState<Page<T>>({
    items: [],
    nextToken: "",
    token: "",
    index: 0,
    size: initialSize,
    history: [],
    loaded: false,
  });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const request = useRef<{ controller: AbortController; id: number } | null>(null);
  const sequence = useRef(0);
  const attempted = useRef<Position>({
    token: "",
    index: 0,
    size: initialSize,
    history: [],
  });

  const load = useCallback(
    async (position: Position) => {
      request.current?.controller.abort();
      const controller = new AbortController();
      const id = ++sequence.current;
      request.current = { controller, id };
      attempted.current = position;
      setLoading(true);
      setError("");
      try {
        const result = await fetchPage(
          { pageToken: position.token, pageSize: position.size },
          controller.signal,
        );
        if (id !== sequence.current || controller.signal.aborted) return;
        setPage({
          ...position,
          items: result.items,
          nextToken: result.nextPageToken,
          loaded: true,
        });
      } catch (error) {
        if (id !== sequence.current || controller.signal.aborted) return;
        setError(error instanceof Error ? error.message : "Unable to load agents.");
      } finally {
        if (id === sequence.current && !controller.signal.aborted) setLoading(false);
      }
    },
    [fetchPage],
  );

  useEffect(() => {
    void load({ token: "", index: 0, size: initialSize, history: [] });
    return () => {
      request.current?.controller.abort();
      ++sequence.current;
    };
  }, [load, initialSize]);

  return {
    ...page,
    loading,
    error,
    next: () =>
      page.nextToken &&
      !loading &&
      void load({
        token: page.nextToken,
        index: page.index + 1,
        size: page.size,
        history: [...page.history, page.token],
      }),
    previous: () =>
      page.history.length > 0 &&
      !loading &&
      void load({
        token: page.history.at(-1)!,
        index: page.index - 1,
        size: page.size,
        history: page.history.slice(0, -1),
      }),
    resize: (size: number) => void load({ token: "", index: 0, size, history: [] }),
    refresh: () => void load(page),
    reset: () => void load({ token: "", index: 0, size: page.size, history: [] }),
    retry: () => void load(attempted.current),
  };
}
