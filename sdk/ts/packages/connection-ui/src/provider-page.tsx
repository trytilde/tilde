import { useState, type ReactNode } from "react";
import { TildeWordmark } from "./wordmark.js";

/** Shared layout for connection setup and identity approval inside sandboxed iframes. */
export function ProviderPage({
  providerName,
  iconUrl,
  title,
  description,
  children,
}: {
  providerName?: string;
  iconUrl?: string;
  title: ReactNode;
  description?: ReactNode;
  children: ReactNode;
}) {
  const [failedIcon, setFailedIcon] = useState<string>();
  return (
    <main className="mx-auto max-w-lg space-y-5 break-words p-6">
      <header className="space-y-5">
        <div className="flex items-center gap-4">
          <TildeWordmark markSize={24} />
          <span aria-hidden="true" className="text-xl text-muted-foreground">
            ×
          </span>
          {iconUrl && iconUrl !== failedIcon ? (
            <img
              src={iconUrl}
              alt={providerName}
              width={32}
              height={32}
              referrerPolicy="no-referrer"
              onError={() => setFailedIcon(iconUrl)}
              className="size-8 object-contain"
            />
          ) : (
            <span
              className="inline-flex size-8 items-center justify-center rounded bg-muted font-semibold"
              aria-hidden="true"
            >
              {providerName?.slice(0, 1)}
            </span>
          )}
        </div>
        <div className="space-y-2">
          <h1 className="text-2xl font-semibold">{title}</h1>
          {description}
        </div>
      </header>
      {children}
    </main>
  );
}
