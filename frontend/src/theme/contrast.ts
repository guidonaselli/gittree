/**
 * WCAG 2.1 relative luminance and contrast ratio calculations.
 * Implements W3C Recommendation: https://www.w3.org/TR/WCAG21/#dfn-relative-luminance
 */

export function parseHexColor(hex: string): [number, number, number] {
  let clean = hex.trim().replace(/^#/, "");
  if (clean.length === 3) {
    clean = clean[0] + clean[0] + clean[1] + clean[1] + clean[2] + clean[2];
  } else if (clean.length === 8) {
    // Drop alpha for solid surface luminance calculations
    clean = clean.slice(0, 6);
  }
  if (clean.length !== 6) {
    throw new Error(`Unsupported hex color format: "${hex}"`);
  }
  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  return [r, g, b];
}

export function srgbChannelToLinear(c: number): number {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
}

export function getRelativeLuminance(r: number, g: number, b: number): number {
  const lr = srgbChannelToLinear(r);
  const lg = srgbChannelToLinear(g);
  const lb = srgbChannelToLinear(b);
  return 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;
}

export function getContrastRatio(foregroundHex: string, backgroundHex: string): number {
  const [r1, g1, b1] = parseHexColor(foregroundHex);
  const [r2, g2, b2] = parseHexColor(backgroundHex);
  const l1 = getRelativeLuminance(r1, g1, b1);
  const l2 = getRelativeLuminance(r2, g2, b2);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

export const WCAG_AA_NORMAL_TEXT = 4.5;
export const WCAG_AA_LARGE_TEXT = 3.0;

export function meetsWcagAaNormal(foregroundHex: string, backgroundHex: string): boolean {
  return getContrastRatio(foregroundHex, backgroundHex) >= WCAG_AA_NORMAL_TEXT;
}

export function meetsWcagAaLarge(foregroundHex: string, backgroundHex: string): boolean {
  return getContrastRatio(foregroundHex, backgroundHex) >= WCAG_AA_LARGE_TEXT;
}
