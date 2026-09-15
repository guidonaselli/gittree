import { describe, expect, it } from "vitest";

const ROW_HEIGHT = 28;
const OVERSCAN = 15;

function computeVirtualRange(
  total: number,
  scrollTop: number,
  viewportHeight: number,
  offsetTop: number,
  activeIdx?: number | null
): { start: number; end: number; topSpacer: number; bottomSpacer: number } {
  if (total <= 40) {
    return { start: 0, end: total, topSpacer: 0, bottomSpacer: 0 };
  }

  const relStartPx = Math.max(0, scrollTop - offsetTop);
  const relEndPx = Math.max(0, scrollTop + viewportHeight - offsetTop);

  let start = Math.max(0, Math.floor(relStartPx / ROW_HEIGHT) - OVERSCAN);
  let end = Math.min(total, Math.ceil(relEndPx / ROW_HEIGHT) + OVERSCAN);

  if (activeIdx != null && activeIdx >= 0) {
    if (activeIdx < start) start = activeIdx;
    if (activeIdx >= end) end = activeIdx + 1;
  }

  const topSpacer = start * ROW_HEIGHT;
  const bottomSpacer = (total - end) * ROW_HEIGHT;
  return { start, end, topSpacer, bottomSpacer };
}

describe("working-copy virtualization math", () => {
  it("leaves small lists un-windowed with zero spacers", () => {
    const res = computeVirtualRange(25, 100, 600, 50);
    expect(res.start).toBe(0);
    expect(res.end).toBe(25);
    expect(res.topSpacer).toBe(0);
    expect(res.bottomSpacer).toBe(0);
  });

  it("calculates window and spacers correctly on a 10 000-item list", () => {
    // Scroll at 2800px (100 rows in), viewport 560px (20 rows)
    const res = computeVirtualRange(10_000, 2800, 560, 0);
    // start should be 100 - 15 = 85
    expect(res.start).toBe(85);
    // end should be (2800 + 560) / 28 + 15 = 120 + 15 = 135
    expect(res.end).toBe(135);
    // rendered rows is only 50 out of 10,000!
    expect(res.end - res.start).toBe(50);
    expect(res.topSpacer).toBe(85 * ROW_HEIGHT);
    expect(res.bottomSpacer).toBe((10_000 - 135) * ROW_HEIGHT);
    expect(res.topSpacer + (res.end - res.start) * ROW_HEIGHT + res.bottomSpacer).toBe(10_000 * ROW_HEIGHT);
  });

  it("guarantees an active expanded row is retained in the rendered slice", () => {
    // Row 10 is active, but viewport is at row 200..250
    const res = computeVirtualRange(5_000, 5600, 560, 0, 10);
    expect(res.start).toBe(10);
    expect(res.end).toBeGreaterThan(200);
  });
});
