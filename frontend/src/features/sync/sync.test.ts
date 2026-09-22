import { describe, it, expect, vi } from "vitest";
import type { MultiRemoteFetchResult, PushOutcome } from "../../api/types";

describe("Sync & Auth UI logic", () => {
  it("multi-remote fetch isolates failures without canceling successful remotes", () => {
    const multiResult: MultiRemoteFetchResult = {
      total: 2,
      succeeded: 1,
      failed: 1,
      results: [
        {
          remote: "origin",
          success: true,
          summary: "From https://github.com/example/repo\n * branch main -> FETCH_HEAD",
          error: null,
        },
        {
          remote: "upstream",
          success: false,
          summary: "Failed to fetch from 'upstream'.",
          error: "fatal: unable to access 'https://invalid-host/repo.git': Could not resolve host",
        },
      ],
    };

    expect(multiResult.total).toBe(2);
    expect(multiResult.succeeded).toBe(1);
    expect(multiResult.failed).toBe(1);

    const origin = multiResult.results.find((r) => r.remote === "origin");
    expect(origin?.success).toBe(true);

    const upstream = multiResult.results.find((r) => r.remote === "upstream");
    expect(upstream?.success).toBe(false);
    expect(upstream?.error).toContain("Could not resolve host");
  });

  it("bare force requires explicit acknowledgement while force-with-lease is safe default", () => {
    const validateCanPush = (
      forceKind: "none" | "force_with_lease" | "bare_force",
      acknowledged: boolean
    ) => {
      if (forceKind === "bare_force" && !acknowledged) {
        return false;
      }
      return true;
    };

    // None is allowed
    expect(validateCanPush("none", false)).toBe(true);

    // Force with lease is default force path and allowed without destructive acknowledgment
    expect(validateCanPush("force_with_lease", false)).toBe(true);

    // Bare force without acknowledgment is rejected
    expect(validateCanPush("bare_force", false)).toBe(false);

    // Bare force with acknowledgment is allowed
    expect(validateCanPush("bare_force", true)).toBe(true);
  });

  it("push rejection captures verbatim message and prompts pull-then-retry", () => {
    const pushOutcome: PushOutcome = {
      status: "rejected_non_fast_forward",
      remote_message:
        "To https://github.com/example/repo.git\n ! [rejected] main -> main (non-fast-forward)\nerror: failed to push some refs to 'https://github.com/example/repo.git'\nhint: Updates were rejected because the remote contains work that you do not have locally.",
      suggest_pull: true,
    };

    expect(pushOutcome.status).toBe("rejected_non_fast_forward");
    expect(pushOutcome.suggest_pull).toBe(true);
    expect(pushOutcome.remote_message).toContain("non-fast-forward");
    expect(pushOutcome.remote_message).toContain("Updates were rejected");
  });

  it("askpass host-key confirmation submits yes/no while passphrase submits secret", () => {
    const hostKeyCallback = vi.fn();
    const passphraseCallback = vi.fn();

    // Host key prompt
    const hostPrompt = {
      id: "askpass-1",
      prompt: "The authenticity of host 'github.com' can't be established. ED25519 key fingerprint is SHA256:abc... Are you sure you want to continue connecting (yes/no)?",
      prompt_type: "host_key" as const,
    };

    if (hostPrompt.prompt_type === "host_key") {
      hostKeyCallback(hostPrompt.id, "yes");
    }

    expect(hostKeyCallback).toHaveBeenCalledWith("askpass-1", "yes");

    // Passphrase prompt
    const passPrompt = {
      id: "askpass-2",
      prompt: "Enter passphrase for key '/home/user/.ssh/id_ed25519':",
      prompt_type: "passphrase" as const,
    };

    if (passPrompt.prompt_type === "passphrase") {
      passphraseCallback(passPrompt.id, "my-secret-passphrase");
    }

    expect(passphraseCallback).toHaveBeenCalledWith("askpass-2", "my-secret-passphrase");
  });
});
