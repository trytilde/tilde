/**
 * The example theme the previews share: one set of tokens per scheme, as a host would pass to a
 * space, a chat and a connection setup alike.
 */
import type { Theme } from "@trytilde/chat-ui";

const shape = { radius: "10px", font: '"Iowan Old Style", Georgia, serif' };
export const light: Theme = {
  scheme: "light",
  tokens: {
    canvas: "#fbf7f0",
    surface: "#f1e9da",
    ink: "#2b211c",
    muted: "#7a6a60",
    line: "color-mix(in srgb, #2b211c 14%, transparent)",
    accent: "#b4532a",
    good: "#2f7d4f",
    critical: "#b3261e",
    ...shape,
  },
};
export const dark: Theme = {
  scheme: "dark",
  tokens: {
    canvas: "#211714",
    surface: "#30211d",
    ink: "#faf9f5",
    muted: "#b9b1ab",
    line: "color-mix(in srgb, #faf9f5 18%, transparent)",
    accent: "#e08a5b",
    good: "#6fcf97",
    critical: "#ff8a80",
    ...shape,
  },
};
/** A per-surface override: the same theme with one token changed for one surface. */
export const withCanvas = (theme: Theme, canvas: string): Theme => ({
  ...theme,
  tokens: { ...theme.tokens, canvas },
});
