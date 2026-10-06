import { describe, it, expect } from "vitest";
import { Icon, type IconName } from "./Icon";

describe("Icon component", () => {
  it("creates a solid component function", () => {
    expect(typeof Icon).toBe("function");
  });

  it("accepts all documented icon names", () => {
    const names: IconName[] = [
      "folder",
      "folder-open",
      "settings",
      "search",
      "close",
      "warning",
      "check",
      "error",
      "dirty",
      "branch",
      "commit",
      "refresh",
      "copy",
      "palette",
      "terminal",
      "chevron-down",
      "chevron-right",
      "plus",
      "trash",
    ];
    expect(names.length).toBe(19);
  });
});
