import { useState } from "react";
import { MessageCircleIcon } from "lucide-react";

/** Provider-authored public branding; unknown or unavailable icons get a neutral fallback. */
export function ProviderIcon({ iconUrl }: { iconUrl?: string }) {
  const [failedUrl, setFailedUrl] = useState<string>();
  return iconUrl && iconUrl !== failedUrl ? (
    <img
      src={iconUrl}
      alt=""
      aria-hidden="true"
      width={20}
      height={20}
      referrerPolicy="no-referrer"
      onError={() => setFailedUrl(iconUrl)}
      className="size-5 shrink-0 rounded-sm object-contain dark:bg-white"
    />
  ) : (
    <MessageCircleIcon aria-hidden="true" className="size-5 shrink-0 text-muted-foreground" />
  );
}
