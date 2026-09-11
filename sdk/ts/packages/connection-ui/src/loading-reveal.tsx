import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import type { ReactNode } from "react";

/** Keep embedded content mounted for its handshake while hiding its initial render. */
export function LoadingReveal({
  loading,
  label,
  children,
  className = "min-h-dvh",
}: {
  loading: boolean;
  label: string;
  children?: ReactNode;
  className?: string;
}) {
  const reducedMotion = useReducedMotion();
  const duration = reducedMotion ? 0 : 0.18;
  return (
    <div className={`relative ${className}`} aria-busy={loading}>
      <motion.div
        className="h-full"
        inert={loading}
        aria-hidden={loading}
        initial={false}
        animate={{ opacity: loading ? 0 : 1 }}
        transition={{ duration, delay: loading ? 0 : duration }}
      >
        {children}
      </motion.div>
      <AnimatePresence initial={false}>
        {loading && (
          <motion.div
            key="loading"
            role="status"
            aria-label={label}
            className="absolute inset-0 flex items-center justify-center gap-1.5 text-muted-foreground"
            initial={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration }}
          >
            {[0, 1, 2].map((dot) => (
              <motion.span
                key={dot}
                aria-hidden="true"
                className="size-1.5 rounded-full bg-current"
                animate={{ y: reducedMotion ? 0 : [0, -5, 0] }}
                transition={{
                  duration: 0.6,
                  repeat: reducedMotion ? 0 : Infinity,
                  delay: dot * 0.12,
                  ease: "easeInOut",
                }}
              />
            ))}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
