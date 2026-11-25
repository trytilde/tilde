import type { CSSProperties } from "react";
import type { Theme, ThemeTokens } from "@trytilde/connection-ui";

export type { Theme, ThemeTokens };

const mix = (colour: string, percent: number) =>
  `color-mix(in srgb, ${colour} ${percent}%, transparent)`;
/** Black or white, whichever reads on a fill of `colour`. */
const onFill = (colour: string) => `oklch(from ${colour} clamp(0, (0.6 - l) * 1000, 1) 0 0)`;

/**
 * A theme as the chat's own variables (style.css holds the defaults), for the `.tilde-chat`
 * root: both palettes its rules read, Dispatch's (`--page`, `--ink`, …) and `--ob-*`. Only given
 * tokens set anything. `radius` and the series colours have no counterpart in the chat.
 */
export function themeStyle(theme: Theme | undefined): CSSProperties | undefined {
  const t: ThemeTokens = theme?.tokens ?? {};
  // Without `muted`, the secondary inks stay tints of the ink, as in the defaults.
  const soft = t.muted ?? (t.ink && mix(t.ink, 60));
  const variables: Record<string, string | undefined> = {
    ...(t.canvas && {
      "--page": t.canvas,
      "--canvas": t.canvas,
      "--rail-surface": t.canvas,
      "--stripe-bg": t.canvas,
      "--ob-bg-base": t.canvas,
      "--ob-bg-subtle": t.canvas,
      "--ob-bg-fade-base": mix(t.canvas, 0),
      "--ob-bg-fade-subtle": mix(t.canvas, 0),
    }),
    ...(t.surface && {
      "--surface": t.surface,
      "--inset": t.surface,
      "--field": t.surface,
      "--bubble-agent": t.surface,
      "--ob-bg-elevated": t.surface,
      "--ob-fill-secondary-solid": t.surface,
      "--ob-fill-secondary-solid-hover": t.surface,
      "--ob-fill-bubble-agent": t.surface,
    }),
    ...(t.ink && {
      "--ink": t.ink,
      "--line-strong": mix(t.ink, 26),
      "--hover": mix(t.ink, 8),
      "--hover-2": mix(t.ink, 14),
      "--ob-text-primary": t.ink,
      "--ob-border-subtle": mix(t.ink, 8),
      "--ob-border-default": mix(t.ink, 26),
      "--ob-border-strong": mix(t.ink, 40),
      "--ob-fill-secondary": mix(t.ink, 10),
      "--ob-fill-secondary-hover": mix(t.ink, 18),
      "--ob-fill-ghost-hover": mix(t.ink, 10),
      "--ob-fill-ghost-selected": mix(t.ink, 18),
    }),
    ...(soft && {
      "--ink-2": soft,
      "--ink-3": mix(soft, 65),
      "--ob-text-secondary": soft,
      "--ob-text-tertiary": mix(soft, 65),
      "--ob-text-disabled": mix(soft, 45),
      "--ob-text-neutral": soft,
    }),
    ...(t.line && { "--line": t.line, "--ob-border-weak": t.line }),
    ...(t.accent && {
      "--accent": t.accent,
      "--accent-ink": t.accent,
      "--accent-tint": mix(t.accent, 17),
      "--bubble-user": t.accent,
      "--bubble-user-ink": onFill(t.accent),
      "--ob-text-accent": t.accent,
      "--ob-text-on-primary": onFill(t.accent),
      "--ob-border-focus": t.accent,
      "--ob-fill-primary": t.accent,
      "--ob-fill-primary-hover": mix(t.accent, 85),
      "--ob-fill-primary-disabled": mix(t.accent, 35),
      "--ob-fill-bubble-user": t.accent,
      "--ob-fill-accent": t.accent,
      "--ob-fill-accent-hover": mix(t.accent, 85),
      "--ob-fill-accent-subtle": mix(t.accent, 17),
    }),
    ...(t.good && {
      "--green": t.good,
      "--green-tint": mix(t.good, 17),
      "--ob-text-success": t.good,
    }),
    ...(t.critical && {
      "--red": t.critical,
      "--red-tint": mix(t.critical, 17),
      "--ob-text-danger": t.critical,
    }),
    ...(t.font && { "--font-sans-face": t.font }),
  };
  return Object.keys(variables).length ? (variables as CSSProperties) : undefined;
}
