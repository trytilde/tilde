// Chat surface pieces from trytilde/dispatch packages/ui (a655a4f): transcript-components.tsx,
// workspace-shell.tsx, chat-components.tsx, chat-composer.tsx and beautiful-ui/blocks/loader-grid.tsx
// (a Beautiful UI fork, MIT). See ../../LICENSE.dispatch and ../../LICENSE.beautiful-ui.
// Tailwind utilities are inlined so hosts need not scan this package.
import { useEffect, useRef, type ReactNode } from "react";
import { ArrowDownIcon, SearchIcon } from "lucide-react";

export function TranscriptTimeSeparator({ dateTime }: { dateTime: string }) {
  const now = new Date();
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  const date = new Date(`${dateTime}T12:00:00`);
  const text =
    dateTime === dayKey(now)
      ? "Today"
      : dateTime === dayKey(yesterday)
        ? "Yesterday"
        : date.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
  return (
    <div className="transcript-time-separator" role="separator">
      <time dateTime={dateTime}>{text}</time>
    </div>
  );
}

/** Local calendar day, YYYY-MM-DD. */
export function dayKey(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

export function ConversationSkeleton() {
  const rows: Array<[string, string, number]> = [
    ["assistant", "wide", 3],
    ["user", "compact", 2],
    ["assistant", "medium", 2],
    ["user", "short", 1],
    ["assistant", "wide", 3],
  ];
  return (
    <div
      aria-busy="true"
      aria-label="Loading conversation"
      className="conversation-loading conversation-skeleton"
      role="status"
    >
      {rows.map(([role, width, lines], index) => (
        <div className={`conversation-skeleton-row ${role}`} aria-hidden="true" key={index}>
          <span className={`conversation-skeleton-bubble ${width}`}>
            {Array.from({ length: lines }, (_, line) => (
              <i key={line} />
            ))}
          </span>
        </div>
      ))}
    </div>
  );
}

export function ScrollToLatestButton({ onClick }: { onClick: () => void }) {
  return (
    <button className="scroll-latest" onClick={onClick} aria-label="Jump to latest" type="button">
      <span>Jump to latest</span>
      <ArrowDownIcon aria-hidden="true" />
    </button>
  );
}

const chevron = Array.from({ length: 9 }, (_, index) => {
  const row = Math.floor(index / 3);
  return (index % 3) * 90 + Math.abs(row - 1) * 90;
});

function LoaderGrid() {
  return (
    <span
      aria-hidden
      style={{ display: "grid", flexShrink: 0, gridTemplateColumns: "repeat(3, 4px)", gap: 1.5 }}
    >
      {chevron.map((delay, index) => (
        <span
          key={index}
          className="loader-grid-cell"
          style={{
            width: 4,
            height: 4,
            borderRadius: 1,
            background: "var(--ink)",
            opacity: 0.15,
            animation: `pixel-on 650ms ease-in-out ${delay}ms infinite`,
          }}
        />
      ))}
    </span>
  );
}

export function ThinkingIndicator({ children }: { children: ReactNode }) {
  return (
    <div className="thinking-inline" role="status">
      <LoaderGrid />
      <span
        style={{
          color: "transparent",
          backgroundClip: "text",
          WebkitBackgroundClip: "text",
          fontSize: 13,
          fontWeight: 500,
          backgroundImage:
            "linear-gradient(90deg, var(--ink-3) 35%, var(--ink) 50%, var(--ink-3) 65%)",
          backgroundSize: "200% 100%",
          animation: "shimmer-text 1.4s linear infinite",
        }}
      >
        {children}
      </span>
    </div>
  );
}

/** Keeps the queue and input in one layout row and tells the conversation how tall it is. */
export function ChatComposerDock({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const element = ref.current;
    const parent = element?.parentElement;
    if (!element || !parent || typeof ResizeObserver === "undefined") return;
    const measure = () =>
      parent.style.setProperty(
        "--composer-dock-height",
        `${element.getBoundingClientRect().height}px`,
      );
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => {
      observer.disconnect();
      parent.style.removeProperty("--composer-dock-height");
    };
  }, []);
  return (
    <div className={`composer-dock chat-prompt ${className}`} ref={ref}>
      {children}
    </div>
  );
}

export interface ChatFindBarProps {
  query: string;
  matchCount: number;
  currentOrdinal: number;
  loading: boolean;
  onQueryChange: (query: string) => void;
  onStepNext: () => void;
  onStepPrevious: () => void;
  onClose: () => void;
}

export function ChatFindBar({
  query,
  matchCount,
  currentOrdinal,
  loading,
  onQueryChange,
  onStepNext,
  onStepPrevious,
  onClose,
}: ChatFindBarProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => inputRef.current?.focus(), []);
  return (
    <div className="chat-find-bar" role="search" aria-label="Find in this chat" aria-busy={loading}>
      <span aria-hidden="true" className="chat-find-icon">
        <SearchIcon aria-hidden />
      </span>
      <input
        aria-label="Search messages"
        onChange={(event) => onQueryChange(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            onClose();
          } else if (event.key === "Enter") {
            event.preventDefault();
            if (event.shiftKey) onStepPrevious();
            else onStepNext();
          }
        }}
        placeholder="Find in conversation"
        ref={inputRef}
        spellCheck={false}
        type="text"
        value={query}
      />
      {query.trim() ? (
        <span className={!loading && matchCount === 0 ? "empty" : ""} role="status">
          {loading ? "…" : `${currentOrdinal}/${matchCount}`}
        </span>
      ) : null}
      <i aria-hidden="true" />
      <button
        type="button"
        aria-label="Previous match"
        disabled={matchCount === 0}
        onClick={onStepPrevious}
      >
        ↑
      </button>
      <button
        type="button"
        aria-label="Next match"
        disabled={matchCount === 0}
        onClick={onStepNext}
      >
        ↓
      </button>
      <button type="button" aria-label="Close find" onClick={onClose}>
        ×
      </button>
    </div>
  );
}
