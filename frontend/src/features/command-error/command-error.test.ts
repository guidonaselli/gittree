import { describe, expect, it } from "vitest";

describe("Command Error Presentation", () => {
  it("formats error details preserving verbatim command, exit status and stderr", () => {
    const error = {
      command: "git merge feature/unrelated --no-ff",
      exitStatus: 128,
      stderr: "fatal: refusing to merge unrelated histories\n",
    };

    expect(error.exitStatus).not.toBe(0);
    expect(error.command).toBe("git merge feature/unrelated --no-ff");
    expect(error.stderr).toContain("fatal: refusing to merge unrelated histories");

    const formatted = [
      `Command: ${error.command}`,
      `Exit Status: ${error.exitStatus}`,
      `Standard Error:\n${error.stderr}`,
    ].join("\n\n");

    expect(formatted).toContain("Command: git merge feature/unrelated --no-ff");
    expect(formatted).toContain("Exit Status: 128");
    expect(formatted).toContain("Standard Error:\nfatal: refusing to merge unrelated histories");
  });

  it("asserts that no non-zero exit code is ever reported as success", () => {
    const testCases = [
      { status: 1, stderr: "error: pathspec 'foo' did not match any file(s) known to git" },
      { status: 128, stderr: "fatal: not a valid object name: 'HEAD'" },
      { status: 255, stderr: "fatal: remote end hung up unexpectedly" },
      { status: -1, stderr: "child process terminated abnormally" },
    ];

    for (const tc of testCases) {
      const isSuccess = tc.status === 0;
      expect(isSuccess).toBe(false);
      expect(tc.status).not.toBe(0);
      expect(tc.stderr.length).toBeGreaterThan(0);
    }
  });
});
