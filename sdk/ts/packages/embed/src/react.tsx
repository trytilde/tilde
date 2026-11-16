"use client";
import { useEffect, useRef, type CSSProperties } from "react";
import {
  mountConnectionSetup,
  mountDashboard,
  type ConnectionSetupOptions,
  type DashboardOptions,
  type MountedDashboard,
} from "./index.js";

export * from "./index.js";
export { Chat, ListSessions, type ChatProps, type ListSessionsProps } from "@trytilde/chat-ui";

/** The chat proxy `@trytilde/embed-server` mounts: pass it as `<Chat proxy={...}>`. */
export const chatProxy = (endpoint = "/api/tilde") => `${endpoint.replace(/\/$/, "")}/chat`;

export type DashboardProps = DashboardOptions & { className?: string; style?: CSSProperties };

/** A dashboard for the signed-in user (see `mountDashboard`). */
export function Dashboard({ className, style, ...options }: DashboardProps) {
  const element = useRef<HTMLDivElement>(null);
  const mounted = useRef<MountedDashboard | null>(null);
  const latest = useRef(options);
  latest.current = options;
  const { endpoint, environment, dashboard } = options;
  useEffect(() => {
    const current = mountDashboard(element.current!, {
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
  }, [endpoint, environment, dashboard]);
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
  const { url, connectionId } = options;
  useEffect(() => {
    const mounted = mountConnectionSetup(element.current!, {
      ...latest.current,
      onComplete: (id) => latest.current.onComplete?.(id),
      onCancel: (id) => latest.current.onCancel?.(id),
      onError: (error) => latest.current.onError?.(error),
    });
    return () => mounted.close();
  }, [url, connectionId]);
  return <div ref={element} className={className} style={style} />;
}
