import { describe, expect, it } from "vitest";
import { pairHunkLines } from "./side-by-side";

describe("pairHunkLines", () => {
  it("pairs a single removal with a single addition", () => {
    const rows = pairHunkLines([" a", "-b", "+b changed", " c"]);
    expect(rows).toEqual([
      { leftIndex: 0, rightIndex: 0, leftText: "a", rightText: "a", isContext: true },
      { leftIndex: 1, rightIndex: 2, leftText: "b", rightText: "b changed", isContext: false },
      { leftIndex: 3, rightIndex: 3, leftText: "c", rightText: "c", isContext: true },
    ]);
  });

  it("leaves an unpaired removal with a blank right cell", () => {
    const rows = pairHunkLines(["-only removed"]);
    expect(rows).toEqual([
      { leftIndex: 0, rightIndex: null, leftText: "only removed", rightText: null, isContext: false },
    ]);
  });

  it("leaves an unpaired addition with a blank left cell", () => {
    const rows = pairHunkLines(["+only added"]);
    expect(rows).toEqual([
      { leftIndex: null, rightIndex: 0, leftText: null, rightText: "only added", isContext: false },
    ]);
  });

  it("pairs uneven runs, blanking the side that runs out first", () => {
    const rows = pairHunkLines(["-r1", "-r2", "+a1"]);
    expect(rows).toEqual([
      { leftIndex: 0, rightIndex: 2, leftText: "r1", rightText: "a1", isContext: false },
      { leftIndex: 1, rightIndex: null, leftText: "r2", rightText: null, isContext: false },
    ]);
  });

  it("keeps context lines identical on both sides", () => {
    const rows = pairHunkLines([" same"]);
    expect(rows[0].leftText).toBe("same");
    expect(rows[0].rightText).toBe("same");
    expect(rows[0].isContext).toBe(true);
  });
});
