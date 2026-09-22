import { describe, expect, it } from "vitest";
import { resolveTheme } from "./apply-palette";
import type { UserThemeInfo } from "../api/types";

describe("resolveTheme", () => {
  it("an explicit theme wins and carries no synced tokens, even with a palette present", () => {
    const result = resolveTheme("dark", { mode: "dark", tokens: { "--color-bg": "#0B0C16" } });
    expect(result).toEqual({ dataTheme: "dark", tokens: {} });
  });

  it("light and dark are both valid explicit overrides", () => {
    expect(resolveTheme("light", undefined).dataTheme).toBe("light");
    expect(resolveTheme("dark", undefined).dataTheme).toBe("dark");
  });

  it("no explicit choice with a palette present adopts the synced mode and tokens", () => {
    const palette = { mode: "light", tokens: { "--color-accent": "#82FB9C" } };
    expect(resolveTheme(null, palette)).toEqual({ dataTheme: "light", tokens: palette.tokens });
    expect(resolveTheme(undefined, palette)).toEqual({ dataTheme: "light", tokens: palette.tokens });
  });

  it("a palette mode other than light falls back to dark", () => {
    const palette = { mode: "dracula", tokens: {} };
    expect(resolveTheme(null, palette).dataTheme).toBe("dark");
  });

  it("no explicit choice and no palette falls back to the base system default", () => {
    expect(resolveTheme(null, undefined)).toEqual({ dataTheme: undefined, tokens: {} });
  });

  it("resolves user theme by id and applies its polarity and custom tokens", () => {
    const userThemes: UserThemeInfo[] = [
      {
        id: "solarized-dark",
        name: "Solarized Dark",
        mode: "dark",
        path: "/path/solarized-dark.toml",
        tokens: {
          "--color-bg": "#002b36",
          "--color-text": "#839496",
        },
      },
    ];

    const res = resolveTheme("solarized-dark", undefined, userThemes);
    expect(res.dataTheme).toBe("dark");
    expect(res.tokens["--color-bg"]).toBe("#002b36");
    expect(res.tokens["--color-text"]).toBe("#839496");
    expect(res.userTheme?.name).toBe("Solarized Dark");
  });

  it("user theme selection overrides desktop palette sync", () => {
    const palette = { mode: "light", tokens: { "--color-bg": "#ffffff" } };
    const userThemes: UserThemeInfo[] = [
      {
        id: "nord",
        name: "Nord",
        mode: "dark",
        path: "/path/nord.toml",
        tokens: { "--color-bg": "#2e3440" },
      },
    ];

    const res = resolveTheme("nord", palette, userThemes);
    expect(res.dataTheme).toBe("dark");
    expect(res.tokens["--color-bg"]).toBe("#2e3440");
  });

  it("unknown user theme falls back safely to system/palette default", () => {
    const palette = { mode: "dark", tokens: { "--color-accent": "#5b9bdb" } };
    const res = resolveTheme("non-existent-theme", palette, []);
    expect(res.dataTheme).toBe("dark");
    expect(res.tokens).toEqual(palette.tokens);
  });
});
