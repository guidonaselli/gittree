import type { DesktopPalette, UserThemeInfo } from "../api/types";

export const ALL_CUSTOM_TOKENS = [
  "--color-bg",
  "--color-bg-subtle",
  "--color-bg-inset",
  "--color-bg-raised",
  "--color-border",
  "--color-border-strong",
  "--color-overlay",
  "--color-text",
  "--color-text-muted",
  "--color-text-faint",
  "--color-accent",
  "--color-accent-hover",
  "--color-accent-text",
  "--color-focus-ring",
  "--color-success",
  "--color-success-bg",
  "--color-warning",
  "--color-warning-bg",
  "--color-danger",
  "--color-danger-bg",
  "--color-info",
  "--color-info-bg",
  "--color-neutral",
  "--color-neutral-bg",
  "--color-diff-add-bg",
  "--color-diff-add-text",
  "--color-diff-del-bg",
  "--color-diff-del-text",
  "--color-diff-context-text",
] as const;

export const PALETTE_TOKENS = ALL_CUSTOM_TOKENS;

export interface ThemeResolution {
  dataTheme: "light" | "dark" | undefined;
  tokens: Record<string, string>;
  userTheme?: UserThemeInfo;
}

export function resolveTheme(
  explicit: string | null | undefined,
  palette: DesktopPalette | undefined,
  userThemes?: UserThemeInfo[],
): ThemeResolution {
  if (explicit === "light" || explicit === "dark") {
    return { dataTheme: explicit, tokens: {} };
  }

  if (explicit && explicit !== "system" && userThemes) {
    const userTheme = userThemes.find(
      (t) => t.id === explicit || `user:${t.id}` === explicit
    );
    if (userTheme) {
      return {
        dataTheme: userTheme.mode,
        tokens: userTheme.tokens,
        userTheme,
      };
    }
  }

  if (palette) {
    return {
      dataTheme: palette.mode === "light" ? "light" : "dark",
      tokens: palette.tokens,
    };
  }

  return { dataTheme: undefined, tokens: {} };
}
