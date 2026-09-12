import { ExternalLinkIcon } from "lucide-react";
export function ExternalLink({ href, label }: { href?: string; label: string }) {
  if (!href || !/^https?:\/\//.test(href)) return null;
  return (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      aria-label={label}
      className="inline-flex shrink-0 items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-xs font-medium hover:bg-muted focus-visible:outline focus-visible:outline-ring"
    >
      {label}
      <ExternalLinkIcon className="size-3" />
    </a>
  );
}
