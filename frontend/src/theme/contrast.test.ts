import { describe, expect, it } from "vitest";
import { BUILTIN_LIGHT_THEME, BUILTIN_DARK_THEME } from "./builtin-theme";
import {
  getContrastRatio,
  parseHexColor,
  WCAG_AA_NORMAL_TEXT,
} from "./contrast";

describe("WCAG AA Contrast Calculations", () => {
  it("computes standard contrast ratios correctly", () => {
    expect(parseHexColor("#fff")).toEqual([255, 255, 255]);
    expect(parseHexColor("#000000")).toEqual([0, 0, 0]);

    const blackOnWhite = getContrastRatio("#000000", "#ffffff");
    expect(blackOnWhite).toBeCloseTo(21.0, 1);

    const whiteOnWhite = getContrastRatio("#ffffff", "#ffffff");
    expect(whiteOnWhite).toBeCloseTo(1.0, 1);
  });

  it("asserts all light theme text token pairs meet WCAG AA contrast (>= 4.5:1)", () => {
    const light = BUILTIN_LIGHT_THEME.tokens;

    // Text token pairs evaluated in light theme
    const textPairs = [
      { name: "body text on default background", fg: light["--color-text"], bg: light["--color-bg"] },
      { name: "body text on subtle background", fg: light["--color-text"], bg: light["--color-bg-subtle"] },
      { name: "body text on inset background", fg: light["--color-text"], bg: light["--color-bg-inset"] },
      { name: "body text on raised background", fg: light["--color-text"], bg: light["--color-bg-raised"] },
      { name: "muted text on default background", fg: light["--color-text-muted"], bg: light["--color-bg"] },
      { name: "muted text on subtle background", fg: light["--color-text-muted"], bg: light["--color-bg-subtle"] },
      { name: "accent text on accent background", fg: light["--color-accent-text"], bg: light["--color-accent"] },
      { name: "success text on success background", fg: light["--color-success"], bg: light["--color-success-bg"] },
      { name: "warning text on warning background", fg: light["--color-warning"], bg: light["--color-warning-bg"] },
      { name: "danger text on danger background", fg: light["--color-danger"], bg: light["--color-danger-bg"] },
      { name: "info text on info background", fg: light["--color-info"], bg: light["--color-info-bg"] },
      { name: "diff add text on diff add background", fg: light["--color-diff-add-text"], bg: light["--color-diff-add-bg"] },
      { name: "diff del text on diff del background", fg: light["--color-diff-del-text"], bg: light["--color-diff-del-bg"] },
      { name: "diff context text on default background", fg: light["--color-diff-context-text"], bg: light["--color-bg"] },
    ];

    for (const pair of textPairs) {
      expect(pair.fg, `Token for ${pair.name} must exist in tokens`).toBeDefined();
      expect(pair.bg, `Background token for ${pair.name} must exist in tokens`).toBeDefined();
      const ratio = getContrastRatio(pair.fg, pair.bg);
      expect(
        ratio,
        `Light theme pair "${pair.name}" (${pair.fg} on ${pair.bg}) must meet WCAG AA >= 4.5:1, got ${ratio.toFixed(2)}:1`,
      ).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
    }
  });

  it("asserts all dark theme text token pairs meet WCAG AA contrast (>= 4.5:1)", () => {
    const dark = BUILTIN_DARK_THEME.tokens;

    // Text token pairs evaluated in dark theme
    const textPairs = [
      { name: "body text on default background", fg: dark["--color-text"], bg: dark["--color-bg"] },
      { name: "body text on subtle background", fg: dark["--color-text"], bg: dark["--color-bg-subtle"] },
      { name: "body text on inset background", fg: dark["--color-text"], bg: dark["--color-bg-inset"] },
      { name: "body text on raised background", fg: dark["--color-text"], bg: dark["--color-bg-raised"] },
      { name: "muted text on default background", fg: dark["--color-text-muted"], bg: dark["--color-bg"] },
      { name: "muted text on subtle background", fg: dark["--color-text-muted"], bg: dark["--color-bg-subtle"] },
      { name: "accent text on accent background", fg: dark["--color-accent-text"], bg: dark["--color-accent"] },
      { name: "success text on success background", fg: dark["--color-success"], bg: dark["--color-success-bg"] },
      { name: "warning text on warning background", fg: dark["--color-warning"], bg: dark["--color-warning-bg"] },
      { name: "danger text on danger background", fg: dark["--color-danger"], bg: dark["--color-danger-bg"] },
      { name: "info text on info background", fg: dark["--color-info"], bg: dark["--color-info-bg"] },
      { name: "diff add text on diff add background", fg: dark["--color-diff-add-text"], bg: dark["--color-diff-add-bg"] },
      { name: "diff del text on diff del background", fg: dark["--color-diff-del-text"], bg: dark["--color-diff-del-bg"] },
      { name: "diff context text on default background", fg: dark["--color-diff-context-text"], bg: dark["--color-bg"] },
    ];

    for (const pair of textPairs) {
      expect(pair.fg, `Token for ${pair.name} must exist in tokens`).toBeDefined();
      expect(pair.bg, `Background token for ${pair.name} must exist in tokens`).toBeDefined();
      const ratio = getContrastRatio(pair.fg, pair.bg);
      expect(
        ratio,
        `Dark theme pair "${pair.name}" (${pair.fg} on ${pair.bg}) must meet WCAG AA >= 4.5:1, got ${ratio.toFixed(2)}:1`,
      ).toBeGreaterThanOrEqual(WCAG_AA_NORMAL_TEXT);
    }
  });
});
