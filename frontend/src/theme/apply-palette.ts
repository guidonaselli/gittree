import type { DesktopPalette } from "../api/types";

export const PALETTE_TOKENS = [
  "--color-accent",
  "--color-focus-ring",
  "--color-bg",
  "--color-bg-inset",
  "--color-bg-subtle",
  "--color-text",
  "--color-text-muted",
  "--color-text-faint",
  "--color-success",
  "--color-danger",
  "--color-warning",
  "--color-info",
] as const;

export interface ThemeResolution {
  dataTheme: "light" | "dark" | undefined;
  tokens: Record<string, string>;
}

export function resolveTheme(
  explicit: string | null | undefined,
  palette: DesktopPalette | undefined,
): ThemeResolution {
  if (explicit === "light" || explicit === "dark") {
    return { dataTheme: explicit, tokens: {} };
  }
  if (palette) {
    return {
      dataTheme: palette.mode === "light" ? "light" : "dark",
      tokens: palette.tokens,
    };
  }
  return { dataTheme: undefined, tokens: {} };
}
