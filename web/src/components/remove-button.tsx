import { useState, type ReactNode } from "react";
import { Trash2Icon } from "lucide-react";
import { Button } from "./ui/button";
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "./ui/alert-dialog";

/**
 * A red trash icon that asks before removing. `onConfirm` runs only once confirmed; the dialog
 * stays open (and disabled) while it runs and closes when it settles, since callers report
 * their own errors.
 */
export function RemoveButton({
  label,
  title,
  description,
  confirmLabel = "Remove",
  onConfirm,
  disabled,
  size = "icon-sm",
}: {
  /** Accessible name and tooltip of the icon, e.g. "Remove Cloudflare". */
  label: string;
  title: string;
  description: ReactNode;
  confirmLabel?: string;
  onConfirm: () => Promise<unknown> | void;
  disabled?: boolean;
  size?: "icon-xs" | "icon-sm";
}) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  async function confirm() {
    setBusy(true);
    try {
      await onConfirm();
    } finally {
      setBusy(false);
      setOpen(false);
    }
  }
  return (
    <>
      <Button
        variant="ghost"
        size={size}
        aria-label={label}
        title={label}
        disabled={disabled || busy}
        className="text-destructive hover:bg-destructive/10 hover:text-destructive dark:hover:bg-destructive/20"
        onClick={() => setOpen(true)}
      >
        <Trash2Icon />
      </Button>
      <AlertDialog open={open} onOpenChange={(next) => !busy && setOpen(next)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{title}</AlertDialogTitle>
            <AlertDialogDescription>{description}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>Cancel</AlertDialogCancel>
            <Button variant="destructive" disabled={busy} onClick={() => void confirm()}>
              {busy ? "Removing…" : confirmLabel}
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
