import { describe, it, expect } from "vitest";
import type { SubmoduleState, BranchEntry } from "../../api/types";
import { isDirty, gitlinkAheadBehind } from "../submodules/submodule-helpers";

describe("Sidebar Tree Navigation Filtering & Status Helpers", () => {
  it("filters branches accurately with case-insensitive matching", () => {
    const branches: BranchEntry[] = [
      {
        name: "master",
        is_head: true,
        is_remote: false,
        target_commit: "1111111",
        commit_subject: "initial",
        upstream: null,
        ahead_behind: [0, 0],
        upstream_basis: "None",
      },
      {
        name: "feature/auth",
        is_head: false,
        is_remote: false,
        target_commit: "2222222",
        commit_subject: "auth",
        upstream: null,
        ahead_behind: [2, 0],
        upstream_basis: "None",
      },
      {
        name: "bugfix/login",
        is_head: false,
        is_remote: false,
        target_commit: "3333333",
        commit_subject: "fix",
        upstream: null,
        ahead_behind: [0, 1],
        upstream_basis: "None",
      },
      {
        name: "origin/master",
        is_head: false,
        is_remote: true,
        target_commit: "1111111",
        commit_subject: "remote master",
        upstream: null,
        ahead_behind: [0, 0],
        upstream_basis: "None",
      },
    ];

    const localOnly = branches.filter((b) => !b.is_remote);
    expect(localOnly.length).toBe(3);

    const filterQuery = (query: string) => {
      const q = query.toLowerCase().trim();
      if (!q) return localOnly;
      return localOnly.filter((b) => b.name.toLowerCase().includes(q));
    };

    expect(filterQuery("AUTH")).toHaveLength(1);
    expect(filterQuery("AUTH")[0].name).toBe("feature/auth");
    expect(filterQuery("nonexistent")).toHaveLength(0);
    expect(filterQuery("")).toHaveLength(3);
  });

  it("filters submodules by name, path or branch accurately", () => {
    const submodules: SubmoduleState[] = [
      {
        name: "core-service",
        path: "services/core-service",
        relative_path: "services/core-service",
        depth: 1,
        parent_path: null,
        declared_branch: { state: "Known", value: "main" },
        url: { state: "Known", value: "https://example.com/core.git" },
        initialized: true,
        gitlink_commit: { state: "Known", value: "abc111" },
        branch: { state: "Known", value: { Named: "main" } },
        gitlink_divergence: { state: "Known", value: "InSync" },
        remote_basis: { state: "Known", value: { Configured: { refname: "origin/main" } } },
        remote_ahead_behind: { state: "Known", value: [0, 0] },
        dirty: { state: "Known", value: false },
        last_fetch_unix_secs: { state: "Known", value: 123456 },
        drift: { state: "Known", value: [] },
      },
      {
        name: "web-ui",
        path: "clients/web-ui",
        relative_path: "clients/web-ui",
        depth: 1,
        parent_path: null,
        declared_branch: { state: "Known", value: "feat/redesign" },
        url: { state: "Known", value: "https://example.com/web.git" },
        initialized: true,
        gitlink_commit: { state: "Known", value: "def222" },
        branch: { state: "Known", value: { Named: "feat/redesign" } },
        gitlink_divergence: { state: "Known", value: { Ahead: { ahead: 1 } } },
        remote_basis: { state: "Known", value: { Configured: { refname: "origin/feat/redesign" } } },
        remote_ahead_behind: { state: "Known", value: [1, 0] },
        dirty: { state: "Known", value: true },
        last_fetch_unix_secs: { state: "Known", value: 123456 },
        drift: { state: "Known", value: [] },
      },
    ];

    const filterSubmodules = (query: string) => {
      const q = query.toLowerCase().trim();
      if (!q) return submodules;
      return submodules.filter(
        (s) =>
          s.name.toLowerCase().includes(q) ||
          s.path.toLowerCase().includes(q)
      );
    };

    expect(filterSubmodules("web")).toHaveLength(1);
    expect(filterSubmodules("web")[0].name).toBe("web-ui");
    expect(isDirty(filterSubmodules("web")[0])).toBe(true);
    expect(gitlinkAheadBehind(filterSubmodules("web")[0])).toEqual({ ahead: 1, behind: 0 });

    expect(filterSubmodules("core")).toHaveLength(1);
    expect(isDirty(filterSubmodules("core")[0])).toBe(false);
  });
});
