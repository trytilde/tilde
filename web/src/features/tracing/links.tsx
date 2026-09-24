import { ExternalLinkIcon } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
export function ExternalLink({
  href,
  label,
  compact = false,
}: {
  href?: string;
  label: string;
  compact?: boolean;
}) {
  if (!href || !/^https?:\/\//.test(href)) return null;
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={label}
            className={`inline-flex shrink-0 items-center justify-center gap-1.5 text-xs font-medium text-muted-foreground underline-offset-4 hover:text-foreground focus-visible:outline focus-visible:outline-ring ${compact ? "size-[26px]" : "py-1.5 hover:underline"}`}
          >
            {!compact && label}
            <ExternalLinkIcon className="size-3.5" />
          </a>
        }
      />
      <TooltipContent side="bottom" className="trace-controls">
        {compact ? "Open in Langfuse" : label}
      </TooltipContent>
    </Tooltip>
  );
}
