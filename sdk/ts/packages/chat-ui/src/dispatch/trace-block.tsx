"use client";

import { useLayoutEffect, useRef, useState, type ReactNode } from "react";

/* ─────────────────────────────────────────────────────────
 * TRACE BLOCK
 * Data-driven fork of the Beautiful UI "Thinking" primitive
 * (beautifului.dev, MIT): expandable agent trace with a
 * shimmering header while work is in flight. The upstream
 * component is demo-driven; this fork takes real rows.
 * ───────────────────────────────────────────────────────── */

export interface TraceRow {
  id: string;
  primary: ReactNode;
  secondary?: string;
  mono?: boolean;
  add?: number;
  del?: number;
  href?: string;
  /** Row shows a spinner instead of a check while pending. */
  pending?: boolean;
  /** Row failed — check swaps to a red cross. */
  failed?: boolean;
  /** Prose row (reasoning) — wraps instead of truncating. */
  prose?: boolean;
  detail?: ReactNode;
}

export interface TraceBlockProps {
  /** Header label while `working`. */
  activeLabel: string;
  /** Header label once settled. */
  doneLabel: string;
  working?: boolean;
  rows: readonly TraceRow[];
  defaultExpanded?: boolean;
  className?: string;
}

export function TraceBlock({
  activeLabel,
  doneLabel,
  working = false,
  rows,
  defaultExpanded = false,
  className,
}: TraceBlockProps) {
  const [manualExpanded, setManualExpanded] = useState<boolean | null>(null);
  const [selectedRow, setSelectedRow] = useState<string | null>(null);
  const expanded = manualExpanded ?? (working || defaultExpanded);
  const traceRef = useRef<HTMLDivElement>(null);
  const [lineHeight, setLineHeight] = useState(0);

  useLayoutEffect(() => {
    if (traceRef.current) setLineHeight(traceRef.current.offsetHeight);
  }, [rows.length, expanded, selectedRow]);

  return (
    <div className={`flex w-full flex-col ${className ?? ""}`}>
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => setManualExpanded((current) => !(current ?? (working || defaultExpanded)))}
        className="-mx-1.5 flex w-fit items-center gap-2 rounded-control px-1.5 py-1
          transition-colors duration-100 hover:bg-hover-2"
      >
        <svg
          aria-hidden
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill={working ? "var(--ink-2)" : "var(--ink-3)"}
        >
          <path d="M12 2l2.4 7.2L22 12l-7.6 2.8L12 22l-2.4-7.2L2 12l7.6-2.8z" />
        </svg>
        <span role="status" className="contents">
          {working ? (
            <span
              className="bg-clip-text text-[13px] font-medium whitespace-nowrap text-transparent"
              style={{
                backgroundImage:
                  "linear-gradient(90deg, var(--ink-3) 35%, var(--ink) 50%, var(--ink-3) 65%)",
                backgroundSize: "200% 100%",
                animation: "shimmer-text 1.4s linear infinite",
              }}
            >
              {activeLabel}
            </span>
          ) : (
            <span
              className="text-[13px] font-medium whitespace-nowrap text-ink-2"
              style={{ animation: "fade-in 350ms ease-out both" }}
            >
              {doneLabel}
            </span>
          )}
        </span>
        <svg
          aria-hidden
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="var(--ink-3)"
          strokeWidth="2.2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="transition-transform duration-300"
          style={{ transform: expanded ? "rotate(180deg)" : "rotate(0)" }}
        >
          <path d="M6 9l6 6 6-6" />
        </svg>
      </button>

      <div
        className="grid transition-[grid-template-rows,opacity] duration-400"
        style={{
          gridTemplateRows: expanded ? "1fr" : "0fr",
          opacity: expanded ? 1 : 0,
          transitionTimingFunction: "cubic-bezier(0.23, 1, 0.32, 1)",
        }}
      >
        <div className="overflow-hidden">
          <div className="relative mt-1 ml-[5px] pl-4">
            <span
              aria-hidden
              className="absolute left-[3px] w-px bg-line"
              style={{
                top: -8,
                height: lineHeight ? lineHeight - 2 : 0,
                transition: "height 500ms cubic-bezier(0.23,1,0.32,1)",
              }}
            />
            <div ref={traceRef} className="flex flex-col gap-1 py-1">
              {rows.map((row, index) => {
                const selected = selectedRow === row.id;
                const rowClass =
                  "flex min-h-7 w-full items-center gap-2 rounded-[6px] px-1.5 py-0.5 text-left";
                const animation = {
                  animation: `fade-up 320ms cubic-bezier(0.23,1,0.32,1) ${index * 120}ms both`,
                };
                const marker = row.prose ? null : row.pending ? (
                  <span
                    className="size-3 shrink-0 rounded-full border-[1.5px] border-line-strong border-t-ink-2"
                    style={{ animation: "spin 700ms linear infinite" }}
                  />
                ) : row.failed ? (
                  <svg
                    aria-hidden
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="var(--red)"
                    strokeWidth="2.5"
                    strokeLinecap="round"
                    className="shrink-0"
                  >
                    <path d="M18 6L6 18M6 6l12 12" />
                  </svg>
                ) : (
                  <svg
                    aria-hidden
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="var(--ink-3)"
                    strokeWidth="2.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    className="shrink-0"
                  >
                    <path d="M20 6L9 17l-5-5" />
                  </svg>
                );
                const content = (
                  <>
                    {marker}
                    <span
                      className={`min-w-0 text-[12.5px] ${
                        row.prose
                          ? "whitespace-normal leading-relaxed text-ink-2"
                          : "truncate font-medium text-ink"
                      } ${row.mono && !row.prose ? "font-mono" : ""}`}
                    >
                      {row.primary}
                    </span>
                    {row.secondary ? (
                      <span
                        className={`shrink-0 text-[11.5px] text-ink-3 ${row.mono ? "font-mono" : ""}`}
                      >
                        {row.secondary}
                      </span>
                    ) : null}
                    {row.add !== undefined ? (
                      <span className="shrink-0 font-mono text-[11px] tabular-nums">
                        <span className="text-green">+{row.add}</span>{" "}
                        <span className="text-red">−{row.del ?? 0}</span>
                      </span>
                    ) : null}
                  </>
                );

                if (row.href) {
                  return (
                    <a
                      key={row.id}
                      href={row.href}
                      target="_blank"
                      rel="noreferrer"
                      className={`${rowClass} transition-colors duration-150 hover:bg-hover`}
                      style={animation}
                    >
                      {content}
                    </a>
                  );
                }
                if (row.detail) {
                  return (
                    <div key={row.id} className="flex flex-col" style={animation}>
                      <button
                        type="button"
                        aria-pressed={selected}
                        onClick={() => setSelectedRow(selected ? null : row.id)}
                        className={`${rowClass} transition-colors duration-150 ${
                          selected ? "bg-inset" : "hover:bg-hover"
                        }`}
                      >
                        {content}
                      </button>
                      {selected ? (
                        <div className="mt-1 overflow-x-auto rounded-[8px] bg-inset p-2.5 font-mono text-[11.5px] leading-relaxed text-ink-2 shadow-hairline">
                          {row.detail}
                        </div>
                      ) : null}
                    </div>
                  );
                }
                return (
                  <div key={row.id} className={rowClass} style={animation}>
                    {content}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
