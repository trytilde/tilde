import { useEffect, useState } from "react";
import { MoonIcon, SunIcon } from "lucide-react";
import { Button } from "@/components/ui/button";

type Theme = "light" | "dark";
const storageKey = "tilde.theme";

function initialTheme(): Theme {
  try {
    const saved = localStorage.getItem(storageKey);
    if (saved === "light" || saved === "dark") return saved;
  } catch {
    // Theme switching still works when browser storage is unavailable.
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(initialTheme);
  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    document
      .querySelector('meta[name="theme-color"]')
      ?.setAttribute("content", theme === "dark" ? "#211714" : "#ffffff");
  }, [theme]);
  const label = theme === "dark" ? "Switch to light mode" : "Switch to dark mode";
  return (
    <Button
      variant="ghost"
      size="icon-sm"
      className="ml-auto mr-2 shrink-0"
      aria-label={label}
      title={label}
      onClick={() => {
        const next = theme === "dark" ? "light" : "dark";
        setTheme(next);
        try {
          localStorage.setItem(storageKey, next);
        } catch {
          /* Keep the in-memory choice. */
        }
      }}
    >
      {theme === "dark" ? <SunIcon /> : <MoonIcon />}
    </Button>
  );
}
