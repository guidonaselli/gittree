import { describe, it, expect } from "vitest";
import {
  isDirty,
  isDetached,
  gitlinkAheadBehind,
  hasNoRemoteBasis,
  hasDrift,
  isFetchStale,
  lastFetchLabel,
  submoduleDrifts,
} from "./submodule-helpers";
import type { SubmoduleState } from "../../api/types";

function mockSubmodule(partial: Partial<SubmoduleState>): SubmoduleState {
  return {
    name: "sub-test",
    path: "/repo/sub-test",
    relative_path: "sub-test",
    depth: 1,
    parent_path: null,
    declared_branch: { state: "Known", value: "main" },
    url: { state: "Known", value: "https://example.com/sub.git" },
    initialized: true,
    gitlink_commit: { state: "Known", value: "abcdef1" },
    branch: { state: "Known", value: { Named: "main" } },
    gitlink_divergence: { state: "Known", value: "InSync" },
    remote_basis: { state: "Known", value: { Configured: { refname: "origin/main" } } },
    remote_ahead_behind: { state: "Known", value: [0, 0] },
    dirty: { state: "Known", value: false },
    last_fetch_unix_secs: { state: "Known", value: Math.floor(Date.now() / 1000) - 3600 },
    drift: { state: "Known", value: [] },
    ...partial,
  };
}

describe("SubmoduleMatrix state helpers", () => {
  it("identifies dirty submodules correctly", () => {
    const clean = mockSubmodule({ dirty: { state: "Known", value: false } });
    const dirty = mockSubmodule({ dirty: { state: "Known", value: true } });
    const unknown = mockSubmodule({ dirty: { state: "Unknown", value: { reason: "uninit" } } });

    expect(isDirty(clean)).toBe(false);
    expect(isDirty(dirty)).toBe(true);
    expect(isDirty(unknown)).toBe(false);
  });

  it("identifies detached HEAD submodules correctly", () => {
    const named = mockSubmodule({ branch: { state: "Known", value: { Named: "feature" } } });
    const detached = mockSubmodule({
      branch: { state: "Known", value: { Detached: { commit: "1234567", pointing_refs: ["v1.0"] } } },
    });

    expect(isDetached(named)).toBe(false);
    expect(isDetached(detached)).toBe(true);
  });

  it("extracts gitlink divergence ahead/behind counts for all variants", () => {
    const inSync = mockSubmodule({ gitlink_divergence: { state: "Known", value: "InSync" } });
    const ahead = mockSubmodule({ gitlink_divergence: { state: "Known", value: { Ahead: { ahead: 3 } } } });
    const behind = mockSubmodule({ gitlink_divergence: { state: "Known", value: { Behind: { behind: 2 } } } });
    const both = mockSubmodule({ gitlink_divergence: { state: "Known", value: { Both: { ahead: 4, behind: 1 } } } });
    const missing = mockSubmodule({
      gitlink_divergence: { state: "Known", value: { GitlinkObjectMissingLocally: { gitlink_commit: "abc" } } },
    });

    expect(gitlinkAheadBehind(inSync)).toBeNull();
    expect(gitlinkAheadBehind(ahead)).toEqual({ ahead: 3, behind: 0 });
    expect(gitlinkAheadBehind(behind)).toEqual({ ahead: 0, behind: 2 });
    expect(gitlinkAheadBehind(both)).toEqual({ ahead: 4, behind: 1 });
    expect(gitlinkAheadBehind(missing)).toBeNull();
  });

  it("identifies missing remote basis", () => {
    const hasBasis = mockSubmodule({
      remote_basis: { state: "Known", value: { Inferred: { refname: "origin/main" } } },
    });
    const noBasis = mockSubmodule({ remote_basis: { state: "Known", value: "None" } });

    expect(hasNoRemoteBasis(hasBasis)).toBe(false);
    expect(hasNoRemoteBasis(noBasis)).toBe(true);
  });

  it("identifies drift correctly", () => {
    const noDrift = mockSubmodule({ drift: { state: "Known", value: [] } });
    const drifted = mockSubmodule({ drift: { state: "Known", value: ["DeclaredButAbsent"] } });

    expect(hasDrift(noDrift)).toBe(false);
    expect(hasDrift(drifted)).toBe(true);
    expect(submoduleDrifts(drifted)).toEqual(["DeclaredButAbsent"]);
  });

  it("detects fetch staleness when never fetched or fetched long ago", () => {
    const now = Math.floor(Date.now() / 1000);
    const fresh = mockSubmodule({ last_fetch_unix_secs: { state: "Known", value: now - 3600 } });
    const stale = mockSubmodule({ last_fetch_unix_secs: { state: "Known", value: now - 100000 } });
    const never = mockSubmodule({ last_fetch_unix_secs: { state: "Known", value: null } });

    expect(isFetchStale(fresh)).toBe(false);
    expect(isFetchStale(stale)).toBe(true);
    expect(isFetchStale(never)).toBe(true);
  });

  it("formats relative last fetch labels", () => {
    const now = Math.floor(Date.now() / 1000);
    const today = mockSubmodule({ last_fetch_unix_secs: { state: "Known", value: now - 60 } });
    const threeDays = mockSubmodule({ last_fetch_unix_secs: { state: "Known", value: now - 3 * 86400 } });
    const never = mockSubmodule({ last_fetch_unix_secs: { state: "Known", value: null } });
    const unknown = mockSubmodule({ last_fetch_unix_secs: { state: "Unknown", value: { reason: "uninit" } } });

    expect(lastFetchLabel(today)).toBe("today");
    expect(lastFetchLabel(threeDays)).toBe("3d ago");
    expect(lastFetchLabel(never)).toBe("never");
    expect(lastFetchLabel(unknown)).toBe("unknown (uninit)");
  });
});
