/**
 * Built-in Light and Dark theme token maps.
 * Used for WCAG AA contrast assertions and as the base fallback
 * for partial user-authored themes.
 */

export interface BuiltinTheme {
  id: "light" | "dark";
  name: string;
  mode: "light" | "dark";
  tokens: Record<string, string>;
}

export const BUILTIN_LIGHT_THEME: BuiltinTheme = {
  id: "light",
  name: "Light (Built-in)",
  mode: "light",
  tokens: {
    "--color-bg": "#ffffff",
    "--color-bg-subtle": "#f4f5f7",
    "--color-bg-inset": "#eceef1",
    "--color-bg-raised": "#ffffff",
    "--color-border": "#d7dbe0",
    "--color-border-strong": "#b7bec7",
    "--color-text": "#1a1d21",
    "--color-text-muted": "#565d66",
    "--color-accent": "#2b6cb0",
    "--color-accent-hover": "#235a92",
    "--color-accent-text": "#ffffff",
    "--color-focus-ring": "#2b6cb0",
    "--color-success": "#15733b",
    "--color-success-bg": "#e6f4ea",
    "--color-warning": "#8a5a00",
    "--color-warning-bg": "#fff3d6",
    "--color-danger": "#b3261e",
    "--color-danger-bg": "#fbe9e7",
    "--color-info": "#2b6cb0",
    "--color-info-bg": "#e8f0fa",
    "--color-neutral": "#565d66",
    "--color-neutral-bg": "#eceef1",
    "--color-diff-add-bg": "#e6f4ea",
    "--color-diff-add-text": "#15733b",
    "--color-diff-del-bg": "#fbe9e7",
    "--color-diff-del-text": "#b3261e",
    "--color-diff-context-text": "#565d66",
  },
};

export const BUILTIN_DARK_THEME: BuiltinTheme = {
  id: "dark",
  name: "Dark (Built-in)",
  mode: "dark",
  tokens: {
    "--color-bg": "#1b1d21",
    "--color-bg-subtle": "#232629",
    "--color-bg-inset": "#17191c",
    "--color-bg-raised": "#26292d",
    "--color-border": "#3b4046",
    "--color-border-strong": "#4f555e",
    "--color-text": "#e7e9ec",
    "--color-text-muted": "#a7aeb6",
    "--color-accent": "#5b9bdb",
    "--color-accent-hover": "#7cb0e3",
    "--color-accent-text": "#0c1116",
    "--color-focus-ring": "#5b9bdb",
    "--color-success": "#4cbb7a",
    "--color-success-bg": "#123522",
    "--color-warning": "#d8a53b",
    "--color-warning-bg": "#3a2c0c",
    "--color-danger": "#e5766c",
    "--color-danger-bg": "#3a1614",
    "--color-info": "#5b9bdb",
    "--color-info-bg": "#142334",
    "--color-neutral": "#a7aeb6",
    "--color-neutral-bg": "#2a2d31",
    "--color-diff-add-bg": "#123522",
    "--color-diff-add-text": "#4cbb7a",
    "--color-diff-del-bg": "#3a1614",
    "--color-diff-del-text": "#e5766c",
    "--color-diff-context-text": "#a7aeb6",
  },
};
