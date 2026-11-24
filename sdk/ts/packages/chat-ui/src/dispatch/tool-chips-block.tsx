"use client";

import {
  ChevronDownIcon,
  FileTextIcon,
  type LucideIcon,
  PencilIcon,
  SparklesIcon,
  SquareTerminalIcon,
  WrenchIcon,
} from "lucide-react";
import { useState } from "react";

/* ─────────────────────────────────────────────────────────
 * TOOL CHIPS BLOCK
 * Data-driven fork of the Beautiful UI "Tool Chips"
 * primitive (beautifului.dev, MIT): an agent run as compact
 * rows — tool label plus an inline chip, hover swaps the
 * icon for a chevron, every row expands to show what the
 * tool actually did.
 * ───────────────────────────────────────────────────────── */

export type ToolChipIcon = "think" | "write" | "run" | "read" | "tool";

const Icons: Record<ToolChipIcon, LucideIcon> = {
  think: SparklesIcon,
  write: PencilIcon,
  run: SquareTerminalIcon,
  read: FileTextIcon,
  tool: WrenchIcon,
};

export interface ToolChipDetailLine {
  text: string;
  tone?: "add" | "del" | "error";
}

export interface ToolChipRow {
  id: string;
  icon?: ToolChipIcon;
  label: string;
  chip?: string;
  mono?: boolean;
  detailMono?: boolean;
  detail?: readonly ToolChipDetailLine[];
  pending?: boolean;
  failed?: boolean;
}

export interface ToolChipsBlockProps {
  headerLabel: string;
  rows: readonly ToolChipRow[];
  defaultOpen?: boolean;
  className?: string;
}

export function ToolChipsBlock({
  headerLabel,
  rows,
  defaultOpen = true,
  className,
}: ToolChipsBlockProps) {
  const [open, setOpen] = useState(defaultOpen);
  const [openRows, setOpenRows] = useState<ReadonlySet<string>>(new Set());

  const toggleRow = (id: string) =>
    setOpenRows((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  return (
    <div className={`w-full ${className ?? ""}`}>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
        className="-mx-1.5 flex w-fit items-center gap-1.5 rounded-control px-1.5 py-1
          text-[12.5px] text-ink-2 transition-colors duration-100 hover:bg-hover-2"
      >
        <ChevronDownIcon
          aria-hidden
          size={12}
          className="transition-transform duration-200"
          style={{ transform: open ? "rotate(0deg)" : "rotate(-90deg)" }}
        />
        <span className="tabular-nums">{headerLabel}</span>
      </button>

      <div
        className="grid transition-[grid-template-rows,opacity] duration-300"
        style={{ gridTemplateRows: open ? "1fr" : "0fr", opacity: open ? 1 : 0 }}
      >
        <div className="-mx-1 overflow-hidden px-1.5 pb-1">
          <div className="mt-1.5 flex flex-col gap-1">
            {rows.map((row) => {
              const rowOpen = openRows.has(row.id);
              const RowIcon = Icons[row.icon ?? "tool"];
              const expandable = Boolean(row.detail?.length);
              return (
                <div
                  key={row.id}
                  style={{ animation: "fade-up 300ms cubic-bezier(0.23,1,0.32,1) both" }}
                >
                  <button
                    type="button"
                    aria-expanded={expandable ? rowOpen : undefined}
                    onClick={expandable ? () => toggleRow(row.id) : undefined}
                    className={`group/row -mx-[3px] flex h-7 w-[calc(100%+6px)] min-w-0 items-center gap-2
                      rounded-control px-[3px] text-left transition-colors duration-100
                      ${expandable ? "hover:bg-hover-2" : "cursor-default"}`}
                  >
                    <span
                      className={`relative flex size-4 shrink-0 items-center justify-center ${
                        row.failed ? "text-red" : "text-ink-3"
                      }`}
                    >
                      {row.pending ? (
                        <span
                          className="size-3 rounded-full border-[1.5px] border-line-strong border-t-ink-2"
                          style={{ animation: "spin 700ms linear infinite" }}
                        />
                      ) : (
                        <>
                          <RowIcon
                            aria-hidden
                            size={13}
                            className={`transition-opacity duration-100 ${
                              expandable ? "group-hover/row:opacity-0" : ""
                            } ${rowOpen ? "opacity-0" : ""}`}
                          />
                          {expandable ? (
                            <ChevronDownIcon
                              aria-hidden
                              size={12}
                              className={`absolute transition-[opacity,transform] duration-150
                                group-hover/row:opacity-100 ${rowOpen ? "opacity-100" : "opacity-0"}`}
                              style={{ transform: rowOpen ? "rotate(0deg)" : "rotate(-90deg)" }}
                            />
                          ) : null}
                        </>
                      )}
                    </span>
                    <span className="shrink-0 text-[12.5px] font-medium text-ink">{row.label}</span>
                    {row.chip ? (
                      <span
                        className={`inline-flex h-5.5 min-w-0 flex-1 items-center truncate rounded-chip
                          bg-field px-1.5 text-[11.5px] text-ink-2 shadow-hairline
                          ${row.mono ? "font-mono" : ""}`}
                      >
                        {row.chip}
                      </span>
                    ) : null}
                  </button>

                  {expandable ? (
                    <div
                      className="grid transition-[grid-template-rows,opacity] duration-300"
                      style={{
                        gridTemplateRows: rowOpen ? "1fr" : "0fr",
                        opacity: rowOpen ? 1 : 0,
                        transitionTimingFunction: "cubic-bezier(0.23, 1, 0.32, 1)",
                      }}
                    >
                      <div className="min-h-0 overflow-hidden">
                        <div className="mt-0.5 mb-1 ml-2 flex flex-col gap-0.5 border-l border-line py-0.5 pl-3.5">
                          {row.detail?.map((line, index) => (
                            <span
                              key={index}
                              className={`truncate text-[11.5px] leading-[1.6] ${
                                row.detailMono ? "font-mono" : ""
                              } ${
                                line.tone === "add"
                                  ? "text-green"
                                  : line.tone === "del" || line.tone === "error"
                                    ? "text-red"
                                    : "text-ink-2"
                              }`}
                            >
                              {line.text}
                            </span>
                          ))}
                        </div>
                      </div>
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
