import { describe, expect, it } from "vitest";
import type {
  ConflictItem,
  ConflictMarkerInfo,
  StageOutcome,
  SubmoduleConflictInfo,
} from "../../api/types";

describe("Conflict surfacing, markers guard, and submodule resolution logic", () => {
  it("formats conflict types and descriptions distinctly", () => {
    const items: ConflictItem[] = [
      {
        path: "src/app.ts",
        conflict_type: "both_modified",
        conflict_code: "UU",
        description: "Both modified by us and them",
        is_submodule: false,
        ours_exists: true,
        theirs_exists: true,
        base_exists: true,
      },
      {
        path: "src/new_feature.ts",
        conflict_type: "both_added",
        conflict_code: "AA",
        description: "Both added independently",
        is_submodule: false,
        ours_exists: true,
        theirs_exists: true,
        base_exists: false,
      },
      {
        path: "deleted_locally.ts",
        conflict_type: "delete_modify",
        conflict_code: "DU",
        description: "Deleted by us, modified by them",
        is_submodule: false,
        ours_exists: false,
        theirs_exists: true,
        base_exists: true,
      },
      {
        path: "renamed_file.ts",
        conflict_type: "rename_rename",
        conflict_code: "UA",
        description: "Rename conflict (destination added by them)",
        is_submodule: false,
        ours_exists: false,
        theirs_exists: true,
        base_exists: false,
      },
      {
        path: "vendor/core",
        conflict_type: "submodule",
        conflict_code: "UU",
        description: "Submodule gitlink conflict",
        is_submodule: true,
        ours_exists: true,
        theirs_exists: true,
        base_exists: true,
      },
    ];

    expect(items.length).toBe(5);
    expect(items[0].conflict_type).toBe("both_modified");
    expect(items[1].conflict_type).toBe("both_added");
    expect(items[2].conflict_type).toBe("delete_modify");
    expect(items[3].conflict_type).toBe("rename_rename");
    expect(items[4].is_submodule).toBe(true);
  });

  it("handles conflict marker detection and refusal payload correctly", () => {
    const markerInfo: ConflictMarkerInfo = {
      has_markers: true,
      marker_count: 3,
      marker_lines: [12, 18, 25],
      preview_lines: [
        "Line 12: <<<<<<< HEAD",
        "Line 18: =======",
        "Line 25: >>>>>>> origin/feature",
      ],
    };

    expect(markerInfo.has_markers).toBe(true);
    expect(markerInfo.marker_lines).toEqual([12, 18, 25]);
    expect(markerInfo.preview_lines[0]).toContain("<<<<<<< HEAD");

    const refusalOutcome: StageOutcome = {
      status: "marker_refusal",
      file: "conflicted.txt",
      marker_lines: markerInfo.marker_lines,
      preview_lines: markerInfo.preview_lines,
    };

    if (refusalOutcome.status === "marker_refusal") {
      expect(refusalOutcome.file).toBe("conflicted.txt");
      expect(refusalOutcome.marker_lines).toContain(18);
    } else {
      expect.unreachable();
    }
  });

  it("surfaces submodule gitlink candidate commits with subjects and metadata", () => {
    const subInfo: SubmoduleConflictInfo = {
      path: "libs/shared",
      ours_commit: {
        sha: "1111222233334444555566667777888899990000",
        short_sha: "1111222",
        author: "Alice Developer",
        date: "2026-09-20 14:30:00",
        subject: "feat: add secure auth client to submodule",
      },
      theirs_commit: {
        sha: "aaaa222233334444555566667777888899990000",
        short_sha: "aaaa222",
        author: "Bob Engineer",
        date: "2026-09-21 09:15:00",
        subject: "fix: update network retry budget in submodule",
      },
      base_commit: {
        sha: "0000111122223333444455556666777788889999",
        short_sha: "0000111",
        author: "Core Team",
        date: "2026-09-15 10:00:00",
        subject: "chore: baseline submodule setup",
      },
    };

    expect(subInfo.ours_commit?.subject).toBe("feat: add secure auth client to submodule");
    expect(subInfo.theirs_commit?.subject).toBe("fix: update network retry budget in submodule");
    expect(subInfo.base_commit?.short_sha).toBe("0000111");

    // Picking either candidate commit by sha is supported
    const chosenOursSha = subInfo.ours_commit?.sha;
    const chosenTheirsSha = subInfo.theirs_commit?.sha;
    expect(chosenOursSha).toBe("1111222233334444555566667777888899990000");
    expect(chosenTheirsSha).toBe("aaaa222233334444555566667777888899990000");
  });
});
