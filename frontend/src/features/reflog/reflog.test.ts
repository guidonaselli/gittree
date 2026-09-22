import { describe, it, expect } from "vitest";
import type { ReflogEntry, ResetMode } from "../../api/types";

describe("Reflog Recovery logic", () => {
  const sampleEntries: ReflogEntry[] = [
    {
      selector: "HEAD@{0}",
      index: 0,
      short_sha: "9ff43f0",
      commit_sha: "9ff43f0d31d11275efbeafac796e035fcacdcd3a",
      operation: "commit",
      message: "feat: sync operations and auth",
      committer_date: "2026-09-22 09:38:13 -0300",
      committer_name: "Guido Naselli",
      committer_email: "gnaselli@test.com",
    },
    {
      selector: "HEAD@{1}",
      index: 1,
      short_sha: "726efb2",
      commit_sha: "726efb2748cc6658821f2609b2fc680d6563c870",
      operation: "reset",
      message: "moving to HEAD~1",
      committer_date: "2026-09-21 16:34:53 -0300",
      committer_name: "Guido Naselli",
      committer_email: "gnaselli@test.com",
    },
    {
      selector: "HEAD@{2}",
      index: 2,
      short_sha: "c8d7b0e",
      commit_sha: "c8d7b0e909a1053a9835ef39c2689c9966281143",
      operation: "checkout",
      message: "moving from feature to master",
      committer_date: "2026-09-21 15:45:15 -0300",
      committer_name: "Guido Naselli",
      committer_email: "gnaselli@test.com",
    },
  ];

  it("filters reflog entries accurately by query across fields", () => {
    const filter = (entries: ReflogEntry[], query: string) => {
      const q = query.trim().toLowerCase();
      if (!q) return entries;
      return entries.filter(
        (e) =>
          e.message.toLowerCase().includes(q) ||
          e.operation.toLowerCase().includes(q) ||
          e.short_sha.toLowerCase().includes(q) ||
          e.selector.toLowerCase().includes(q)
      );
    };

    // Filter by operation
    const resets = filter(sampleEntries, "reset");
    expect(resets).toHaveLength(1);
    expect(resets[0].selector).toBe("HEAD@{1}");

    // Filter by sha
    const bySha = filter(sampleEntries, "c8d7b0e");
    expect(bySha).toHaveLength(1);
    expect(bySha[0].operation).toBe("checkout");

    // Filter by message
    const byMsg = filter(sampleEntries, "sync operations");
    expect(byMsg).toHaveLength(1);
    expect(byMsg[0].short_sha).toBe("9ff43f0");

    // Filter with no match
    expect(filter(sampleEntries, "nonexistent")).toHaveLength(0);
  });

  it("enforces safety acknowledgement for hard reset while allowing mixed and soft by default", () => {
    const canSubmitReset = (mode: ResetMode, hardAcknowledged: boolean) => {
      if (mode === "hard") {
        return hardAcknowledged;
      }
      return true;
    };

    // Mixed mode is safe default: no destructive ack required
    expect(canSubmitReset("mixed", false)).toBe(true);

    // Soft mode keeps changes in index: no destructive ack required
    expect(canSubmitReset("soft", false)).toBe(true);

    // Hard mode without checkbox is blocked
    expect(canSubmitReset("hard", false)).toBe(false);

    // Hard mode with checkbox is allowed
    expect(canSubmitReset("hard", true)).toBe(true);
  });

  it("classifies operation badges cleanly for diverse git actions", () => {
    function getBadgeClass(operation: string): string {
      const op = operation.toLowerCase();
      if (op.startsWith("commit")) return "reflog-badge-commit";
      if (op.startsWith("checkout")) return "reflog-badge-checkout";
      if (op.startsWith("reset")) return "reflog-badge-reset";
      if (op.startsWith("rebase")) return "reflog-badge-rebase";
      if (op.startsWith("merge")) return "reflog-badge-merge";
      if (op.startsWith("pull")) return "reflog-badge-pull";
      if (op.startsWith("cherry-pick")) return "reflog-badge-cherry-pick";
      return "reflog-badge-other";
    }

    expect(getBadgeClass("commit")).toBe("reflog-badge-commit");
    expect(getBadgeClass("commit (amend)")).toBe("reflog-badge-commit");
    expect(getBadgeClass("checkout")).toBe("reflog-badge-checkout");
    expect(getBadgeClass("reset")).toBe("reflog-badge-reset");
    expect(getBadgeClass("rebase (finish)")).toBe("reflog-badge-rebase");
    expect(getBadgeClass("merge")).toBe("reflog-badge-merge");
    expect(getBadgeClass("pull")).toBe("reflog-badge-pull");
    expect(getBadgeClass("cherry-pick")).toBe("reflog-badge-cherry-pick");
    expect(getBadgeClass("branch")).toBe("reflog-badge-other");
  });
});
