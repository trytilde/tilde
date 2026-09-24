import { useEffect, useState } from "react";
import { KeyRoundIcon } from "lucide-react";
import { tildeChat } from "@/client";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";

/** The one action on an agent's system-managed Tilde connection: reveal or rotate its key. */
export function TildeChatKey({ agentId, disabled }: { agentId: string; disabled?: boolean }) {
  const [open, setOpen] = useState(false);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    setOpen(false);
    setKey("");
    setError("");
    setCopied(false);
  }, [agentId]);
  async function load(rotate = false) {
    setBusy(true);
    setError("");
    setCopied(false);
    try {
      const response = rotate
        ? await tildeChat.rotateCredentials({ agentId })
        : await tildeChat.getCredentials({ agentId });
      setKey(response.apiKey);
    } catch (error) {
      setError(error instanceof Error ? error.message : "Unable to load key");
    } finally {
      setBusy(false);
    }
  }
  return (
    <>
      <button
        type="button"
        aria-label="Reveal API key"
        title="Reveal API key"
        disabled={disabled}
        onClick={() => {
          setOpen(true);
          void load();
        }}
        className="inline-flex size-6 cursor-pointer items-center justify-center rounded text-muted-foreground hover:text-foreground disabled:cursor-default disabled:opacity-50 [&_svg]:size-3.5"
      >
        <KeyRoundIcon />
      </button>
      <Dialog
        open={open}
        onOpenChange={(next) => {
          if (busy) return;
          setOpen(next);
          if (!next) {
            setKey("");
            setError("");
            setCopied(false);
          }
        }}
      >
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Tilde chat API key</DialogTitle>
            <DialogDescription>
              Embed chat in your application using the Tilde provider. Keep this key on your server;
              your proxy authenticates users and supplies their identity.
            </DialogDescription>
          </DialogHeader>
          {error && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
          <Input
            aria-label="Tilde chat API key"
            type="password"
            readOnly
            value={key}
            placeholder={busy ? "Loading…" : ""}
            autoComplete="off"
          />
          <p className="text-xs text-muted-foreground">
            Rotating the key rejects new requests using the previous key.
          </p>
          <DialogFooter>
            <Button variant="outline" disabled={busy || !key} onClick={() => void load(true)}>
              Rotate key
            </Button>
            <Button
              disabled={busy || !key}
              onClick={() => {
                void navigator.clipboard
                  .writeText(key)
                  .then(() => setCopied(true))
                  .catch(() => setError("Unable to copy key"));
              }}
            >
              {copied ? "Copied" : "Copy key"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
