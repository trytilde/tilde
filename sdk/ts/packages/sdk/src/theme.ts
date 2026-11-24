/**
 * The one theme every embeddable Tilde UI takes: spaces, chat and connection setup. Each UI
 * derives its own variables from these tokens; a token left out keeps that UI's default.
 */

/** Colours and shape; any CSS value. Unknown tokens are ignored. */
export interface ThemeTokens {
  canvas?: string;
  surface?: string;
  ink?: string;
  muted?: string;
  line?: string;
  accent?: string;
  good?: string;
  critical?: string;
  "series-1"?: string;
  "series-2"?: string;
  "series-3"?: string;
  "series-4"?: string;
  "series-5"?: string;
  radius?: string;
  font?: string;
}
export interface Theme {
  tokens?: ThemeTokens;
  scheme?: "light" | "dark";
}
