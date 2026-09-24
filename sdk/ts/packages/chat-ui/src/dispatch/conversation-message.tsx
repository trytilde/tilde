// From trytilde/dispatch chat-components.tsx; trace source action and timestamp label added.
import type { ReactNode, SVGProps } from "react";
export interface ConversationMessageProps {
  role: string;
  createdAt: string;
  timestampLabel?: string;
  onInspect?: () => void;
  continuedPrevious?: boolean;
  continuedNext?: boolean;
  /** Message is attachments only — the bubble renders bare. */
  mediaOnly?: boolean;
  children: ReactNode;
  menuOpen?: boolean;
  onReply?: () => void;
  onToggleMenu?: () => void;
  onStartThread?: () => void;
  onCopy?: () => void;
}

export function ConversationMessage({
  role,
  createdAt,
  timestampLabel,
  onInspect,
  continuedPrevious = false,
  continuedNext = false,
  mediaOnly = false,
  children,
  menuOpen = false,
  onReply,
  onToggleMenu,
  onStartThread,
  onCopy,
}: ConversationMessageProps) {
  return (
    <article
      aria-label={role === "user" ? "Your message" : "Agent message"}
      className={`message ${role} ${continuedPrevious ? "continued-previous" : "group-start"} ${continuedNext ? "continued-next" : ""} ${mediaOnly ? "media-only" : ""}`}
    >
      <div className="message-bubble">{children}</div>
      <div className="message-footer">
        <time dateTime={createdAt}>{timestampLabel ?? formatTime(createdAt)}</time>
        {onInspect && (
          <button className="trace-source" type="button" onClick={onInspect}>
            View trace
          </button>
        )}
      </div>
      {onReply || onToggleMenu ? (
        <div className="message-actions">
          {onReply ? (
            <button aria-label="Reply" onClick={onReply}>
              <ReplyIcon />
            </button>
          ) : null}
          {onToggleMenu ? (
            <button aria-label="More message actions" onClick={onToggleMenu}>
              <MoreIcon />
            </button>
          ) : null}
          {menuOpen ? (
            <div className="message-menu" role="menu">
              {onStartThread ? (
                <button role="menuitem" onClick={onStartThread}>
                  Start a thread
                </button>
              ) : null}
              {onCopy ? (
                <button role="menuitem" onClick={onCopy}>
                  Copy
                </button>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}
    </article>
  );
}

function formatTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.valueOf())
    ? ""
    : date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
export function MoreIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16" {...props}>
      <circle cx="3.25" cy="8" r="1" />
      <circle cx="8" cy="8" r="1" />
      <circle cx="12.75" cy="8" r="1" />
    </svg>
  );
}

export function ReplyIcon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg aria-hidden="true" viewBox="0 0 16 16" {...props}>
      <path d="m6.75 4-4 4 4 4M3.25 8h5.5c2.25 0 3.75 1.2 4 3.5" />
    </svg>
  );
}
