import type { Theme, ThemeTokens } from "@trytilde/sdk/theme";

const mix = (colour: string, percent: number) =>
  `color-mix(in srgb, ${colour} ${percent}%, transparent)`;
/** Black or white, whichever reads on a fill of `colour`. */
const onFill = (colour: string) => `oklch(from ${colour} clamp(0, (0.6 - l) * 1000, 1) 0 0)`;

/** The page's variables each token sets (style.css holds the defaults). */
const derive: { [K in keyof ThemeTokens]: (value: string) => Record<string, string> } = {
  canvas: (v) => ({ "--background": v }),
  surface: (v) => ({ "--card": v, "--popover": v, "--secondary": v, "--muted": v, "--accent": v }),
  ink: (v) => ({
    "--foreground": v,
    "--card-foreground": v,
    "--popover-foreground": v,
    "--secondary-foreground": v,
    "--accent-foreground": v,
    "--input": mix(v, 30),
  }),
  muted: (v) => ({ "--muted-foreground": v }),
  line: (v) => ({ "--border": v }),
  accent: (v) => ({ "--primary": v, "--primary-foreground": onFill(v), "--ring": v }),
  good: (v) => ({ "--success": v }),
  critical: (v) => ({ "--destructive": v }),
  radius: (v) => ({ "--radius": v }),
  font: (v) => ({ "--font-body": v, "--font-display": v }),
};
const all = Object.values(derive).flatMap((variables) => Object.keys(variables("")));

/**
 * Apply a host's theme to this document: its tokens as the page's shadcn variables, its scheme as
 * the `dark` class. Variables of tokens the theme leaves out return to the stylesheet's values.
 */
export function applyTheme(theme: Theme, root: HTMLElement = document.documentElement) {
  const variables: Record<string, string> = {};
  for (const [token, value] of Object.entries(theme.tokens ?? {})) {
    const variablesOf = derive[token as keyof ThemeTokens];
    if (variablesOf && typeof value === "string") Object.assign(variables, variablesOf(value));
  }
  for (const name of all) {
    if (name in variables) root.style.setProperty(name, variables[name]);
    else root.style.removeProperty(name);
  }
  if (theme.scheme) root.classList.toggle("dark", theme.scheme === "dark");
}
