"use client";
import { useEffect, useRef, type CSSProperties } from "react";
import {
  mountConnectionSetup,
  mountSpace,
  type ConnectionSetupOptions,
  type SpaceOptions,
  type MountedSpace,
  type MountedConnectionSetup,
} from "./index.js";

export * from "./index.js";
// `Chat` and `ListSessions` take the same `theme` as `Space` and `ConnectionSetup`.
export { Chat, ListSessions, type ChatProps, type ListSessionsProps } from "@trytilde/chat-ui";

/** The chat proxy `@trytilde/embed-server` mounts: pass it as `<Chat proxy={...}>`. */
export const chatProxy = (endpoint = "/api/tilde") => `${endpoint.replace(/\/$/, "")}/chat`;

export type SpaceProps = SpaceOptions & { className?: string; style?: CSSProperties };

/** A space for the signed-in user (see `mountSpace`). */
export function Space({ className, style, ...options }: SpaceProps) {
  const element = useRef<HTMLDivElement>(null);
  const mounted = useRef<MountedSpace | null>(null);
  const latest = useRef(options);
  latest.current = options;
  const { endpoint, environment, space } = options;
  useEffect(() => {
    const current = mountSpace(element.current!, {
      ...latest.current,
      onNavigate: (to) => latest.current.onNavigate?.(to),
      onEvent: (name, detail) => latest.current.onEvent?.(name, detail),
      onError: (error) => latest.current.onError?.(error),
    });
    mounted.current = current;
    return () => {
      current.unmount();
      mounted.current = null;
    };
  }, [endpoint, environment, space]);
  const theme = JSON.stringify(options.theme ?? null);
  useEffect(() => {
    if (latest.current.theme) mounted.current?.setTheme(latest.current.theme);
  }, [theme]);
  return <div ref={element} className={className} style={style} />;
}

export type ConnectionSetupProps = ConnectionSetupOptions & {
  className?: string;
  style?: CSSProperties;
};

/** Tilde's setup page for one connection, in a frame (see `mountConnectionSetup`). */
export function ConnectionSetup({ className, style, ...options }: ConnectionSetupProps) {
  const element = useRef<HTMLDivElement>(null);
  const latest = useRef(options);
  latest.current = options;
  const mounted = useRef<MountedConnectionSetup | null>(null);
  const { url, connectionId } = options;
  useEffect(() => {
    const current = mountConnectionSetup(element.current!, {
      ...latest.current,
      onComplete: (id) => latest.current.onComplete?.(id),
      onCancel: (id) => latest.current.onCancel?.(id),
      onError: (error) => latest.current.onError?.(error),
    });
    mounted.current = current;
    return () => {
      current.close();
      mounted.current = null;
    };
  }, [url, connectionId]);
  const theme = JSON.stringify(options.theme ?? null);
  useEffect(() => {
    if (latest.current.theme) mounted.current?.setTheme(latest.current.theme);
  }, [theme]);
  return <div ref={element} className={className} style={style} />;
}
