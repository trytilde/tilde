/**
 * Tilde in your application's pages: spaces, connection setup and (from
 * `@trytilde/embed/react`) agent chats, all talking to your server's `@trytilde/embed-server`
 * handler, which holds the embed key and says who the signed-in user is.
 */

import type { Theme } from "@trytilde/chat-ui";

/** One theme for every embedded Tilde UI: spaces, chat and connection setup. */
export type { Theme, ThemeTokens } from "@trytilde/chat-ui";

export interface SpaceOptions {
  /** Where your server mounted `@trytilde/embed-server`. Defaults to `/api/tilde`. */
  endpoint?: string;
  environment: string;
  space: string;
  theme?: Theme;
  /** Size the frame to its content (default true). Frames built before this keep `height`. */
  autoResize?: boolean;
  /** The frame's height until it reports one (default `600px`). */
  height?: string;
  /** A page's `navigate(to)`: a link your app should follow. */
  onNavigate?: (to: string) => void;
  /** A page's `emit(name, detail)`: an action your app handles. */
  onEvent?: (name: string, detail: unknown) => void;
  onError?: (error: Error) => void;
  /** Requests to your server; defaults to `fetch` with same-origin credentials. */
  fetch?: typeof globalThis.fetch;
}

export interface MountedSpace {
  frame: HTMLIFrameElement;
  setTheme(theme: Theme): void;
  unmount(): void;
}

interface SpaceSession {
  frameUrl: string;
  token: string;
  expiresAt: number;
  title: string;
}

export class EmbedError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

const base = (endpoint?: string) => (endpoint ?? "/api/tilde").replace(/\/$/, "");

async function request<T>(
  fetcher: typeof globalThis.fetch,
  url: string,
  init: RequestInit = {},
): Promise<T> {
  const response = await fetcher(url, {
    credentials: "same-origin",
    cache: "no-store",
    ...init,
    headers: init.body ? { "content-type": "application/json" } : undefined,
  });
  const data = (await response.json().catch(() => ({}))) as T & { message?: string };
  if (!response.ok) throw new EmbedError(response.status, data.message ?? "Request failed");
  return data;
}

/**
 * Put a space in `element`: a sandboxed frame of Tilde's spaces host, given short-lived
 * tokens for your signed-in user (renewed before they expire). The frame's origin is opaque, so
 * messages go to its window with target `*` and only its window's messages are read.
 */
export function mountSpace(element: HTMLElement, options: SpaceOptions): MountedSpace {
  const fetcher = options.fetch ?? globalThis.fetch.bind(globalThis);
  const session = () =>
    request<SpaceSession>(fetcher, `${base(options.endpoint)}/spaces/session`, {
      method: "POST",
      body: JSON.stringify({ environment: options.environment, space: options.space }),
    });
  const frame = document.createElement("iframe");
  // allow-forms: React forms need submit events; the frame's CSP still blocks submissions.
  frame.setAttribute("sandbox", "allow-scripts allow-forms");
  frame.setAttribute("referrerpolicy", "no-referrer");
  frame.style.border = "0";
  frame.style.width = "100%";
  frame.style.height = options.height ?? "600px";
  let theme = options.theme;
  let first: Promise<SpaceSession> | undefined = session();
  let closed = false;
  const fail = (error: unknown) =>
    options.onError?.(error instanceof Error ? error : new Error(String(error)));
  const post = (message: unknown) => frame.contentWindow?.postMessage(message, "*");
  function onMessage(event: MessageEvent) {
    if (event.source !== frame.contentWindow || !event.data || typeof event.data !== "object")
      return;
    const data = event.data as Record<string, unknown>;
    switch (data.tilde) {
      case "resize":
        if (options.autoResize !== false && typeof data.height === "number")
          frame.style.height = `${Math.max(0, Math.ceil(data.height))}px`;
        return;
      case "navigate":
        options.onNavigate?.(String(data.to));
        return;
      case "event":
        options.onEvent?.(String(data.name), data.detail);
        return;
      case "ready":
      case "token-request": {
        if (data.tilde === "ready" && theme)
          post({ tilde: "theme", tokens: theme.tokens ?? {}, scheme: theme.scheme });
        const target = event.source;
        const next = first ?? session();
        first = undefined;
        next.then((s) => {
          if (!closed && frame.contentWindow === target)
            post({ tilde: "token", token: s.token, expiresAt: s.expiresAt });
        }, fail);
      }
    }
  }
  window.addEventListener("message", onMessage);
  first.then((s) => {
    if (!closed) {
      frame.title = s.title;
      frame.src = s.frameUrl;
      element.appendChild(frame);
    }
  }, fail);
  return {
    frame,
    setTheme(next) {
      theme = next;
      post({ tilde: "theme", tokens: next.tokens ?? {}, scheme: next.scheme });
    },
    unmount() {
      closed = true;
      window.removeEventListener("message", onMessage);
      frame.remove();
    },
  };
}

export interface ConnectionSetup {
  connectionId: string;
  setupUrl: string;
  expiresAt: number;
}
export interface UserConnection {
  id: string;
  providerId: string;
  name: string;
  status: string;
}

/** Your user's own connections (accounts they connected for an agent's tools). */
export function connections(
  options: { endpoint?: string; agent?: string; fetch?: typeof globalThis.fetch } = {},
) {
  const fetcher = options.fetch ?? globalThis.fetch.bind(globalThis);
  const root = `${base(options.endpoint)}/connections`;
  const agent = options.agent ? `?agent=${encodeURIComponent(options.agent)}` : "";
  return {
    list: () =>
      request<{ connections: UserConnection[] }>(fetcher, `${root}${agent}`).then(
        (r) => r.connections,
      ),
    setup: (provider: string) =>
      request<ConnectionSetup>(fetcher, `${root}/setup`, {
        method: "POST",
        body: JSON.stringify({ provider, agent: options.agent }),
      }),
    reconnect: (id: string) =>
      request<ConnectionSetup>(fetcher, `${root}/${encodeURIComponent(id)}/reconnect${agent}`, {
        method: "POST",
      }),
    remove: (id: string) =>
      request<unknown>(fetcher, `${root}/${encodeURIComponent(id)}${agent}`, {
        method: "DELETE",
      }).then(() => undefined),
  };
}

export interface ConnectionSetupOptions {
  /** A setup page from `connections().setup()` or the server's `createConnectionSetup`. */
  url: string;
  connectionId: string;
  onComplete?: (connectionId: string) => void;
  onCancel?: (connectionId: string) => void;
  onError?: (error: Error) => void;
  /** The frame's height (default `640px`). */
  height?: string;
  theme?: Theme;
}
export interface MountedConnectionSetup {
  /** The frame, or the popup window. */
  target: HTMLIFrameElement | Window;
  setTheme(theme: Theme): void;
  close(): void;
}

/**
 * Show Tilde's connection setup page in `element` (a frame) or in a popup (`"popup"`, from a
 * click handler). Tilde hosts the provider's form and OAuth redirects; credentials never pass
 * through your app. The page takes `theme` when it loads and again on `setTheme`. Completion is announced by the page's window, matched by window and
 * connection id.
 */
export function mountConnectionSetup(
  target: HTMLElement | "popup",
  options: ConnectionSetupOptions,
): MountedConnectionSetup {
  let opened: HTMLIFrameElement | Window;
  if (target === "popup") {
    const popup = window.open(options.url, "tilde-connection-setup", "popup,width=520,height=720");
    if (!popup) {
      options.onError?.(new Error("The popup was blocked"));
      return { target: window, setTheme() {}, close() {} };
    }
    opened = popup;
  } else {
    const frame = document.createElement("iframe");
    frame.src = options.url;
    frame.title = "Connect an account";
    frame.style.border = "0";
    frame.style.width = "100%";
    frame.style.height = options.height ?? "640px";
    frame.setAttribute("allow", "clipboard-write");
    target.appendChild(frame);
    opened = frame;
  }
  const source = () => (opened instanceof HTMLIFrameElement ? opened.contentWindow : opened);
  let done = false;
  let theme = options.theme;
  const post = (next: Theme) =>
    source()?.postMessage({ type: "tilde.connection.theme", theme: next }, "*");
  function close() {
    done = true;
    window.removeEventListener("message", onMessage);
    if (opened instanceof HTMLIFrameElement) opened.remove();
    else opened.close();
  }
  function onMessage(event: MessageEvent) {
    const data = event.data as { type?: string; connectionId?: string } | null;
    if (done || event.source !== source() || !data) return;
    // The page asks before it knows its connection, so this one is matched by window alone.
    if (data.type === "tilde.connection.ready") {
      if (theme) post(theme);
      return;
    }
    if (data.connectionId !== options.connectionId) return;
    if (data.type === "tilde.connection.complete") {
      close();
      options.onComplete?.(options.connectionId);
    } else if (data.type === "tilde.connection.cancelled") {
      close();
      options.onCancel?.(options.connectionId);
    }
  }
  window.addEventListener("message", onMessage);
  return {
    target: opened,
    setTheme(next) {
      theme = next;
      post(next);
    },
    close,
  };
}
