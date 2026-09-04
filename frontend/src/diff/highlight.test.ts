import { describe, expect, it } from "vitest";
import { highlightLine } from "./highlight";

describe("highlightLine", () => {
  it("classifies a keyword", () => {
    const tokens = highlightLine("const x = 1;");
    expect(tokens.find((t) => t.text === "const")?.class).toBe("tok-keyword");
  });

  it("classifies a double-quoted string as a single token", () => {
    const tokens = highlightLine('greet("hello world")');
    expect(tokens.some((t) => t.class === "tok-string" && t.text === '"hello world"')).toBe(true);
  });

  it("classifies a line comment to end of line", () => {
    const tokens = highlightLine("x = 1 // trailing note");
    expect(tokens.some((t) => t.class === "tok-comment" && t.text === "// trailing note")).toBe(true);
  });

  it("classifies a hash comment", () => {
    const tokens = highlightLine("x = 1  # trailing note");
    expect(tokens.some((t) => t.class === "tok-comment" && t.text === "# trailing note")).toBe(true);
  });

  it("classifies an integer and a decimal number", () => {
    const tokens = highlightLine("a = 42 + 3.14");
    expect(tokens.some((t) => t.class === "tok-number" && t.text === "42")).toBe(true);
    expect(tokens.some((t) => t.class === "tok-number" && t.text === "3.14")).toBe(true);
  });

  it("leaves an ordinary identifier unclassified", () => {
    const tokens = highlightLine("myVariable");
    expect(tokens).toEqual([{ text: "myVariable", class: null }]);
  });

  it("reconstructs the original line by concatenating all token text", () => {
    const line = '  if (a > b) { return "yes"; } // check';
    const tokens = highlightLine(line);
    expect(tokens.map((t) => t.text).join("")).toBe(line);
  });
});
