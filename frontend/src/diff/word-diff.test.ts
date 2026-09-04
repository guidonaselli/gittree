import { describe, expect, it } from "vitest";
import { wordDiff } from "./word-diff";

describe("wordDiff", () => {
  it("marks nothing changed for identical lines", () => {
    const { oldSpans, newSpans } = wordDiff("const x = 1;", "const x = 1;");
    expect(oldSpans.every((s) => !s.changed)).toBe(true);
    expect(newSpans.every((s) => !s.changed)).toBe(true);
  });

  it("marks only the single changed word", () => {
    const { oldSpans, newSpans } = wordDiff("let value = 1;", "let value = 2;");
    const changedOld = oldSpans.filter((s) => s.changed).map((s) => s.text);
    const changedNew = newSpans.filter((s) => s.changed).map((s) => s.text);
    expect(changedOld).toEqual(["1"]);
    expect(changedNew).toEqual(["2"]);
  });

  it("marks a pure insertion as entirely new on the added side", () => {
    const { oldSpans, newSpans } = wordDiff("foo()", "foo(bar)");
    expect(oldSpans.every((s) => !s.changed)).toBe(true);
    expect(newSpans.some((s) => s.changed && s.text === "bar")).toBe(true);
  });

  it("marks a pure deletion as entirely removed on the old side", () => {
    const { oldSpans, newSpans } = wordDiff("foo(bar)", "foo()");
    expect(oldSpans.some((s) => s.changed && s.text === "bar")).toBe(true);
    expect(newSpans.every((s) => !s.changed)).toBe(true);
  });

  it("handles empty lines without throwing", () => {
    const { oldSpans, newSpans } = wordDiff("", "");
    expect(oldSpans).toEqual([]);
    expect(newSpans).toEqual([]);
  });

  it("reconstructs the original text by concatenating all spans", () => {
    const oldLine = "  const total = a + b; // sum";
    const newLine = "  const total = a - b; // difference";
    const { oldSpans, newSpans } = wordDiff(oldLine, newLine);
    expect(oldSpans.map((s) => s.text).join("")).toBe(oldLine);
    expect(newSpans.map((s) => s.text).join("")).toBe(newLine);
  });
});
