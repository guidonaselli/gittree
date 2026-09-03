import { describe, expect, it } from "vitest";
import { resolveTheme } from "./apply-palette";

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
});
