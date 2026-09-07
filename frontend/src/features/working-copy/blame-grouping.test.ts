import { describe, expect, it } from "vitest";
import { groupBlameLines } from "./blame-grouping";
import type { BlameLine } from "../../api/commands";

function line(overrides: Partial<BlameLine>): BlameLine {
  return {
    sha: "a".repeat(40),
    author_name: "Alice",
    author_email: "a@example.com",
    author_time: 0,
    summary: "commit",
    orig_line_no: 1,
    final_line_no: 1,
    content: "x",
    is_boundary: false,
    ...overrides,
  };
}

describe("groupBlameLines", () => {
  it("merges consecutive lines from the same commit into one block", () => {
    const blocks = groupBlameLines([
      line({ final_line_no: 1, content: "a" }),
      line({ final_line_no: 2, content: "b" }),
    ]);
    expect(blocks).toHaveLength(1);
    expect(blocks[0].lines).toHaveLength(2);
  });

  it("splits into separate blocks when the commit changes", () => {
    const blocks = groupBlameLines([
      line({ sha: "a".repeat(40), final_line_no: 1 }),
      line({ sha: "b".repeat(40), final_line_no: 2 }),
    ]);
    expect(blocks).toHaveLength(2);
    expect(blocks[0].sha).toBe("a".repeat(40));
    expect(blocks[1].sha).toBe("b".repeat(40));
  });

  it("re-splits when the same commit reappears after a different one", () => {
    const blocks = groupBlameLines([
      line({ sha: "a".repeat(40), final_line_no: 1 }),
      line({ sha: "b".repeat(40), final_line_no: 2 }),
      line({ sha: "a".repeat(40), final_line_no: 3 }),
    ]);
    expect(blocks).toHaveLength(3);
  });

  it("returns an empty array for no lines", () => {
    expect(groupBlameLines([])).toEqual([]);
  });
});
