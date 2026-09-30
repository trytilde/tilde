import { useLayoutEffect, useRef, useState } from "react";
import { cn } from "cn";
import { WrapTextIcon } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "./ui/tooltip";

/**
 * One line of text, clipped with an ellipsis; when it is clipped, a wrap button beside it shows
 * the whole text (and folds it back).
 */
export function ExpandableText({ text, className }: { text: string; className?: string }) {
  const line = useRef<HTMLSpanElement>(null);
  const [clipped, setClipped] = useState(false);
  const [expanded, setExpanded] = useState(false);
  useLayoutEffect(() => {
    const el = line.current;
    if (!el || expanded) return;
    const measure = () => setClipped(el.scrollWidth > el.clientWidth);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [text, expanded]);
  const label = expanded ? "Collapse" : "Expand";
  return (
    <span className="flex min-w-0 items-start gap-1">
      <span
        ref={line}
        className={cn("min-w-0", expanded ? "whitespace-normal" : "truncate", className)}
      >
        {text}
      </span>
      {(clipped || expanded) && (
        <Tooltip>
          <TooltipTrigger
            render={
              <button
                type="button"
                aria-label={label}
                aria-expanded={expanded}
                onClick={(event) => {
                  // Rows around descriptions often navigate on click.
                  event.stopPropagation();
                  setExpanded(!expanded);
                }}
                className="flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground [&_svg]:size-3.5"
              />
            }
          >
            <WrapTextIcon />
          </TooltipTrigger>
          <TooltipContent>{label}</TooltipContent>
        </Tooltip>
      )}
    </span>
  );
}
