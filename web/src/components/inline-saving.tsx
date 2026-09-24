import { useEffect, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { CheckIcon, LoaderCircleIcon, XIcon } from "lucide-react";
import { cn } from "cn";

export type InlineSavingState = "idle" | "saving" | "success" | "error";

/** Compact save feedback. Increment resetKey for each attempt, including immediately resolved saves. */
export function InlineSaving({
  state,
  label = "Changes",
  error,
  resetKey = 0,
  holdMs = 1600,
  className,
}: {
  state: InlineSavingState;
  label?: string;
  error?: string;
  resetKey?: number;
  holdMs?: number;
  className?: string;
}) {
  const reducedMotion = useReducedMotion();
  const [dismissed, setDismissed] = useState<string>();
  const resultKey = `${state}:${resetKey}`;
  const visible = state !== "idle" && (state === "saving" || dismissed !== resultKey);
  useEffect(() => {
    if (state === "saving") {
      setDismissed(undefined);
      return;
    }
    if (state !== "success" && state !== "error") return;
    const timer = setTimeout(() => setDismissed(resultKey), holdMs);
    return () => clearTimeout(timer);
  }, [state, resultKey, holdMs]);
  const announcement = !visible
    ? undefined
    : state === "saving"
      ? `Saving ${label}`
      : state === "success"
        ? `${label} saved`
        : `${label} failed to save`;
  return (
    <span
      role="status"
      aria-live="polite"
      aria-atomic="true"
      aria-label={announcement}
      title={visible ? (error ?? announcement) : undefined}
      data-save-state={visible ? state : "idle"}
      className={cn(
        "relative inline-flex size-3.5 shrink-0 items-center justify-center align-middle",
        className,
      )}
    >
      <span className="sr-only">{visible ? (error ?? announcement) : null}</span>
      <AnimatePresence initial={false}>
        {visible && (
          <motion.span
            key={resultKey}
            aria-hidden="true"
            className="absolute inset-0 inline-flex items-center justify-center"
            initial={{ opacity: 0, scale: reducedMotion ? 1 : 0.8 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: reducedMotion ? 0 : 0.2 }}
          >
            {state === "saving" ? (
              <motion.span
                className="inline-flex text-muted-foreground"
                animate={{ rotate: reducedMotion ? 0 : 360 }}
                transition={{
                  duration: 0.75,
                  repeat: reducedMotion ? 0 : Infinity,
                  ease: "linear",
                }}
              >
                <LoaderCircleIcon className="size-3.5" />
              </motion.span>
            ) : state === "success" ? (
              <CheckIcon className="size-3.5 text-success" strokeWidth={2.5} />
            ) : (
              <XIcon className="size-3.5 text-destructive" strokeWidth={2.5} />
            )}
          </motion.span>
        )}
      </AnimatePresence>
    </span>
  );
}
