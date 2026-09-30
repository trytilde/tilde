import { useRef, useState } from "react";
import { LoadingReveal } from "@trytilde/connection-ui";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";

export type Brokering = { title: string; url: string };
/** The hosted setup page in a sandboxed iframe; only the host holds the setup token. */
export function ConnectionSetupDialog({
  setup,
  error,
  onRetry,
  onClose,
}: {
  setup: Brokering | null;
  error: string;
  onRetry?: () => void;
  onClose: () => void;
}) {
  const [loaded, setLoaded] = useState(false);
  const frame = useRef<HTMLIFrameElement>(null);
  return (
    <Dialog
      open={!!setup}
      onOpenChange={(open) => {
        if (!open) {
          setLoaded(false);
          onClose();
        }
      }}
    >
      <DialogContent className="gap-0 overflow-hidden p-0 sm:max-w-2xl" showCloseButton={false}>
        <DialogTitle className="sr-only">{setup?.title}</DialogTitle>
        <LoadingReveal
          loading={!loaded && !error}
          label="Loading connection setup"
          className="h-[min(680px,75dvh)]"
        >
          {error ? (
            <div className="space-y-3 p-5">
              <p role="alert" className="text-destructive">
                {error}
              </p>
              {onRetry && <Button onClick={onRetry}>Retry setup</Button>}
            </div>
          ) : setup?.url ? (
            <iframe
              ref={frame}
              onLoad={() => setLoaded(true)}
              title={setup.title}
              src={setup.url}
              referrerPolicy="no-referrer"
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-popups-to-escape-sandbox"
              className="h-full w-full border-0"
            />
          ) : null}
        </LoadingReveal>
      </DialogContent>
    </Dialog>
  );
}
