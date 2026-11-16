// From trytilde/dispatch packages/ui/src/chat-components.tsx (a655a4f). See ../../LICENSE.dispatch.
// Added: a group sender line, the quoted reply, hover Reply/Copy actions, and trace navigation.
import type { ReactNode } from "react";
import { MoreIcon, ReplyIcon } from "./icons.js";

export interface ConversationMessageProps {
  role: string;
  createdAt: string;
  timestampLabel?: string;
  messageId?: string;
  tone?: "warning";
  /** Shown above the first bubble of a group, e.g. the sender in a multi-party chat. */
  sender?: ReactNode;
  /** The message this one replies to. */
  quote?: ReactNode;
  attachments?: ReactNode;
  notice?: ReactNode;
  onInspect?: () => void;
  continuedPrevious?: boolean;
  continuedNext?: boolean;
  /** Message is attachments only — the bubble renders bare. */
  mediaOnly?: boolean;
  children?: ReactNode;
  menuOpen?: boolean;
  onReply?: () => void;
  onToggleMenu?: () => void;
  onCopy?: () => void;
}

export function ConversationMessage({
  role,
  createdAt,
  timestampLabel,
  messageId,
  tone,
  sender,
  quote,
  attachments,
  notice,
  onInspect,
  continuedPrevious = false,
  continuedNext = false,
  mediaOnly = false,
  children,
  menuOpen = false,
  onReply,
  onToggleMenu,
  onCopy,
}: ConversationMessageProps) {
  return (
    <article
      data-message-id={messageId}
      data-tone={tone}
      aria-label={role === "user" ? "Your message" : "Agent message"}
      className={`message ${role} ${continuedPrevious ? "continued-previous" : "group-start"} ${continuedNext ? "continued-next" : ""} ${mediaOnly ? "media-only" : ""}`}
    >
      {sender && !continuedPrevious ? <div className="message-sender">{sender}</div> : null}
      {quote ? <blockquote className="message-quote">{quote}</blockquote> : null}
      {children ? <div className="message-bubble">{children}</div> : null}
      {attachments ? <div className="message-attachments">{attachments}</div> : null}
      {notice ? <div className="message-notice">{notice}</div> : null}
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
            <button type="button" aria-label="Reply" title="Reply" onClick={onReply}>
              <ReplyIcon />
            </button>
          ) : null}
          {onToggleMenu ? (
            <button
              type="button"
              aria-label="More message actions"
              aria-expanded={menuOpen}
              onClick={onToggleMenu}
            >
              <MoreIcon className="more-icon" />
            </button>
          ) : null}
          {menuOpen ? (
            <div className="message-menu" role="menu">
              {onCopy ? (
                <button type="button" role="menuitem" onClick={onCopy}>
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

export function formatTime(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.valueOf())
    ? ""
    : date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
}
