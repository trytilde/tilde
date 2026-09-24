/** Shared Tilde mark and wordmark for navigation and provider setup branding. */
export function TildeWordmark({ markSize = 20 }: { markSize?: number }) {
  return (
    <span className="inline-flex shrink-0 items-center gap-2" aria-label="Tilde">
      <svg width={markSize} height={markSize} viewBox="0 0 100 100" fill="none" aria-hidden="true">
        <rect x="4" y="4" width="92" height="92" stroke="currentColor" strokeWidth="8" />
        <path d="M20 50H80" stroke="currentColor" strokeWidth="8" />
      </svg>
      <span className="font-mono text-xl tracking-wide">tilde</span>
    </span>
  );
}
