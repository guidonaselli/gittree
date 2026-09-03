import { describe, expect, it } from "vitest";
import {
  branchLabel,
  gitlinkDivergenceLabel,
  isKnown,
  unknownReason,
  upstreamBasisLabel,
  type Resolved,
  type Branch,
  type GitlinkDivergence,
} from "./types";

describe("Resolved helpers", () => {
  it("isKnown narrows a Known value", () => {
    const r: Resolved<number> = { state: "Known", value: 42 };
    expect(isKnown(r)).toBe(true);
  });

  it("unknownReason surfaces the reason, never silently blank", () => {
    const r: Resolved<number> = { state: "Unknown", value: { reason: "no upstream configured" } };
    expect(isKnown(r)).toBe(false);
    expect(unknownReason(r)).toBe("no upstream configured");
  });
});

describe("branchLabel", () => {
  it("renders a named branch as-is", () => {
    const branch: Resolved<Branch> = { state: "Known", value: { Named: "dev" } };
    expect(branchLabel(branch)).toBe("dev");
  });

  it("renders detached HEAD with a short commit, never an invented branch name", () => {
    const branch: Resolved<Branch> = {
      state: "Known",
      value: { Detached: { commit: "4339d89eded302efb0e16838d09b060b9732a15f" } },
    };
    expect(branchLabel(branch)).toBe("detached @ 4339d89");
  });

  it("surfaces the unresolved reason instead of rendering blank", () => {
    const branch: Resolved<Branch> = { state: "Unknown", value: { reason: "rev-parse HEAD failed" } };
    expect(branchLabel(branch)).toBe("unknown (rev-parse HEAD failed)");
  });
});

describe("upstreamBasisLabel", () => {
  it("labels a configured @{u} as configured, not inferred", () => {
    const { label, inferred } = upstreamBasisLabel({ Configured: { refname: "origin/dev" } });
    expect(label).toBe("origin/dev");
    expect(inferred).toBe(false);
  });

  it("labels a same-named remote branch fallback as inferred", () => {
    const { label, inferred } = upstreamBasisLabel({ Inferred: { refname: "origin/test" } });
    expect(label).toBe("origin/test");
    expect(inferred).toBe(true);
  });

  it("labels the no-basis case distinctly from either configured or inferred", () => {
    const { label, inferred } = upstreamBasisLabel("None");
    expect(label).toBe("no remote basis");
    expect(inferred).toBe(false);
  });
});

describe("gitlinkDivergenceLabel", () => {
  it("distinguishes in-sync from any divergence", () => {
    const d: Resolved<GitlinkDivergence> = { state: "Known", value: "InSync" };
    expect(gitlinkDivergenceLabel(d)).toBe("in sync");
  });

  it("reports ahead/behind counts for a diverged submodule", () => {
    const d: Resolved<GitlinkDivergence> = { state: "Known", value: { Diverged: { ahead: 15, behind: 13 } } };
    expect(gitlinkDivergenceLabel(d)).toBe("15 ahead / 13 behind");
  });

  it("distinguishes a missing gitlink object from zero divergence", () => {
    const d: Resolved<GitlinkDivergence> = { state: "Known", value: "GitlinkObjectMissingLocally" };
    expect(gitlinkDivergenceLabel(d)).toBe("gitlink commit unknown locally");
  });

  it("distinguishes unrelated histories from zero divergence", () => {
    const d: Resolved<GitlinkDivergence> = { state: "Known", value: "UnrelatedHistories" };
    expect(gitlinkDivergenceLabel(d)).toBe("unrelated histories");
  });

  it("never silently blanks an unresolved divergence", () => {
    const d: Resolved<GitlinkDivergence> = { state: "Unknown", value: { reason: "rev-list failed" } };
    expect(gitlinkDivergenceLabel(d)).toBe("unknown (rev-list failed)");
  });
});
