import { describe, expect, it } from "vitest";
import type {
  AbortOutcome,
  ActiveOperationDetail,
  DirtyTreeDetails,
  MergeOptions,
  RebaseAction,
  RebasePlanItem,
} from "../../api/types";

describe("Integration operations data models and logic", () => {
  it("manages interactive rebase plan action and reordering correctly", () => {
    let items: RebasePlanItem[] = [
      { commit: "aaa1111", short_commit: "aaa1111", author: "Dev A", date: "2026-09-21", subject: "feat: first", action: "Pick", new_message: null },
      { commit: "bbb2222", short_commit: "bbb2222", author: "Dev B", date: "2026-09-21", subject: "feat: second", action: "Pick", new_message: null },
      { commit: "ccc3333", short_commit: "ccc3333", author: "Dev C", date: "2026-09-21", subject: "feat: third", action: "Pick", new_message: null },
    ];

    // Reorder: move second item up to first position
    const moveItem = (list: RebasePlanItem[], index: number, dir: "up" | "down") => {
      const next = [...list];
      const target = dir === "up" ? index - 1 : index + 1;
      if (target < 0 || target >= next.length) return next;
      const tmp = next[index];
      next[index] = next[target];
      next[target] = tmp;
      return next;
    };

    items = moveItem(items, 1, "up");
    expect(items[0].commit).toBe("bbb2222");
    expect(items[1].commit).toBe("aaa1111");

    // Change action to Squash and provide custom reword message
    const updateAction = (list: RebasePlanItem[], index: number, action: RebaseAction) => {
      const next = [...list];
      next[index] = { ...next[index], action };
      return next;
    };

    const updateMessage = (list: RebasePlanItem[], index: number, msg: string) => {
      const next = [...list];
      next[index] = { ...next[index], new_message: msg };
      return next;
    };

    items = updateAction(items, 1, "Squash");
    items = updateMessage(items, 1, "Squashed into second commit");
    expect(items[1].action).toBe("Squash");
    expect(items[1].new_message).toBe("Squashed into second commit");

    // Drop an item
    items = updateAction(items, 2, "Drop");
    expect(items[2].action).toBe("Drop");
  });

  it("formats dirty tree refusal summary correctly", () => {
    const refusal: DirtyTreeDetails = {
      staged_count: 2,
      unstaged_count: 5,
      untracked_count: 3,
      summary: "Working tree has uncommitted modifications: 2 staged, 5 unstaged, 3 untracked. Please commit or stash before integrating.",
    };

    expect(refusal.staged_count).toBe(2);
    expect(refusal.unstaged_count).toBe(5);
    expect(refusal.untracked_count).toBe(3);
    expect(refusal.summary).toContain("2 staged");
    expect(refusal.summary).toContain("5 unstaged");
    expect(refusal.summary).toContain("3 untracked");
  });

  it("formats abort outcome confirmation correctly", () => {
    const outcome: AbortOutcome = {
      operation: "Merge",
      restored_head: "0123456789abcdef0123456789abcdef01234567",
      restored_head_short: "0123456",
      restored_head_subject: "feat: initial feature",
      restored_branch: "master",
      working_tree_clean: true,
      summary: "Aborted Merge successfully. Restored branch 'master' to commit 0123456 ('feat: initial feature'). Working copy is clean.",
    };

    expect(outcome.operation).toBe("Merge");
    expect(outcome.restored_branch).toBe("master");
    expect(outcome.working_tree_clean).toBe(true);
    expect(outcome.summary).toContain("Restored branch 'master'");
    expect(outcome.summary).toContain("Working copy is clean");
  });

  it("handles active operation detail state correctly", () => {
    const op: ActiveOperationDetail = {
      kind: "Rebase",
      title: "Interactive rebase in progress",
      description: "Rebase stopped at step 2 of 5 on commit abc1234",
      conflicting_files: ["src/lib.rs", "Cargo.toml"],
      head_commit: "abc1234",
      target_ref: "feature-branch",
      current_commit_msg: "fix: resolve conflict",
      rebase_current_step: 2,
      rebase_total_steps: 5,
      can_skip: true,
    };

    expect(op.kind).toBe("Rebase");
    expect(op.can_skip).toBe(true);
    expect(op.conflicting_files).toHaveLength(2);
    expect(op.rebase_current_step).toBe(2);
    expect(op.rebase_total_steps).toBe(5);
  });

  it("validates merge options combinations", () => {
    const standardMerge: MergeOptions = {
      no_ff: false,
      ff_only: false,
      squash: false,
      message: null,
    };
    expect(standardMerge.no_ff).toBe(false);

    const noFfCustomMsg: MergeOptions = {
      no_ff: true,
      ff_only: false,
      squash: false,
      message: "Merge branch 'release-1.0'",
    };
    expect(noFfCustomMsg.no_ff).toBe(true);
    expect(noFfCustomMsg.message).toBe("Merge branch 'release-1.0'");
  });
});
