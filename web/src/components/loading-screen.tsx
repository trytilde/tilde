import { useEffect, useState } from "react";
import { cn } from "cn";

// Tilde brand fills, cycled like the trytilde/api loading screen.
const FILL_COLORS = ["#0038AE", "#C1630F", "#032E1A", "#3E2723", "#7A697D", "#B91D1A"];

/**
 * The Tilde mark flashing through the brand colors, shown wherever a page, tab or panel waits
 * for its content; `LoadingScreen` is the same loader filling the whole window at startup.
 */
export function TildeLoader({ size = 40, className }: { size?: number; className?: string }) {
  const [index, setIndex] = useState(0);
  useEffect(() => {
    const interval = setInterval(() => setIndex((i) => (i + 1) % FILL_COLORS.length), 600);
    return () => clearInterval(interval);
  }, []);
  const color = FILL_COLORS[index];
  const transition = "all 0.4s ease-in-out";
  return (
    <div
      className={cn("flex flex-col items-center justify-center gap-3 py-10", className)}
      role="status"
      aria-label="Loading"
    >
      <svg width={size} height={size} viewBox="0 0 100 100" fill="none" aria-hidden="true">
        <rect
          x="4"
          y="4"
          width="92"
          height="92"
          fill={color}
          stroke={color}
          strokeWidth="8"
          style={{ transition }}
        />
        <line
          x1="20"
          y1="50"
          x2="80"
          y2="50"
          stroke="#F8F8F3"
          strokeWidth="8"
          strokeLinecap="square"
        />
      </svg>
      <span className="font-mono text-xs tracking-widest" style={{ color, transition }}>
        LOADING
      </span>
    </div>
  );
}

/** The full-page Tilde loading treatment shown while the app starts. */
export function LoadingScreen() {
  return (
    <main className="flex min-h-screen flex-col bg-background">
      <TildeLoader size={48} className="flex-1" />
    </main>
  );
}
