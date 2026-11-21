import { useState } from "react";
import type { Provider } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { Capability } from "@trytilde/contracts/tilde/types/v1/connections_pb.js";
import { ConnectionSetupDialog, type Brokering } from "./connection-setup-dialog";
import { ProviderIcon } from "./provider-icon";
import { AccountSetupDialog } from "./tool-account-setup";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";

/** The providers whose connections launch sandboxes. */
export const SANDBOX_PROVIDERS = ["e2b", "modal"] as const;

/**
 * A new connection to one of `providers`, set up as the catalog sets one up, with no agent
 * involved: the provider is chosen here, then its methods offering `capability`. `onConnected`
 * gets the connection once its setup finishes.
 */
export function NewConnectionDialog({
  open,
  title,
  providers,
  capability = Capability.TOOL,
  onClose,
  onConnected,
}: {
  open: boolean;
  title: string;
  providers: Provider[];
  capability?: Capability;
  onClose: () => void;
  onConnected: (connectionId: string) => void;
}) {
  const [provider, setProvider] = useState<Provider | null>(null);
  const [frame, setFrame] = useState<{ setup: Brokering; connectionId: string } | null>(null);
  return (
    <>
      <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>{title}</DialogTitle>
          </DialogHeader>
          <div className="grid max-h-[60dvh] gap-2 overflow-y-auto">
            {providers.map((candidate) => (
              <button
                key={candidate.id}
                type="button"
                className="flex items-center gap-3 rounded-lg border p-3 text-left hover:bg-muted/50"
                onClick={() => {
                  onClose();
                  setProvider(candidate);
                }}
              >
                <ProviderIcon iconUrl={candidate.iconUrl} />
                <span className="text-sm font-medium">{candidate.name}</span>
              </button>
            ))}
          </div>
        </DialogContent>
      </Dialog>
      {provider && (
        <AccountSetupDialog
          provider={provider}
          capability={capability}
          iconId={provider.id}
          onStarted={() => {}}
          onComplete={(connectionId) => {
            setProvider(null);
            onConnected(connectionId);
          }}
          onCustom={(setup, connectionId) => {
            setProvider(null);
            setFrame({ setup, connectionId });
          }}
          onClose={() => setProvider(null)}
        />
      )}
      {frame && (
        <ConnectionSetupDialog
          setup={frame.setup}
          error=""
          onClose={() => {
            onConnected(frame.connectionId);
            setFrame(null);
          }}
        />
      )}
    </>
  );
}
