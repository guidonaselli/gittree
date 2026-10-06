import { type Component, For, Show, createMemo, createResource, createSignal } from "solid-js";
import {
  checkoutBranch,
  compareBranches,
  createBranch,
  createTrackingBranch,
  deleteBranch,
  getBranches,
  renameBranch,
  stashAndCheckout,
} from "../../api/commands";
import {
  upstreamBasisLabel,
  type BranchEntry,
  type CheckoutOutcome,
  type CommitSummary,
  type DeleteBranchOutcome,
} from "../../api/types";
import "./branches.css";
import { MergeModal } from "../integration/MergeModal";
import { Icon } from "../../ui/Icon";

export interface BranchesViewProps {
  root: string;
  onCheckoutSuccess?: (branch: string) => void;
  onNavigateWorkingCopy?: () => void;
  onOpenReflog?: (branchName: string) => void;
}

export const BranchesView: Component<BranchesViewProps> = (props) => {
  const [branches, { refetch: refetchBranches }] = createResource(
    () => props.root,
    (root) => getBranches(root)
  );

  const [filter, setFilter] = createSignal("");
  const [errorMessage, setErrorMessage] = createSignal<string | null>(null);
  const [actionNotice, setActionNotice] = createSignal<string | null>(null);

  // Modals state
  const [showCreateModal, setShowCreateModal] = createSignal(false);
  const [createTrackingTarget, setCreateTrackingTarget] = createSignal<string | null>(null);
  const [renameTarget, setRenameTarget] = createSignal<BranchEntry | null>(null);
  const [compareTarget, setCompareTarget] = createSignal<{ base: string; target: string } | null>(null);
  const [mergeTarget, setMergeTarget] = createSignal<string | null>(null);

  // Guard Modals state
  const [checkoutConflict, setCheckoutConflict] = createSignal<{
    target: string;
    conflicting_files: string[];
    message: string;
  } | null>(null);

  const [unmergedGuard, setUnmergedGuard] = createSignal<{
    branch: string;
    tip_commit: string;
    commits: CommitSummary[];
    recovery_hint: string;
  } | null>(null);

  const localBranches = createMemo(() => {
    const list = branches()?.filter((b) => !b.is_remote) ?? [];
    const query = filter().toLowerCase().trim();
    if (!query) return list;
    return list.filter((b) => b.name.toLowerCase().includes(query));
  });

  const remoteBranches = createMemo(() => {
    const list = branches()?.filter((b) => b.is_remote) ?? [];
    const query = filter().toLowerCase().trim();
    if (!query) return list;
    return list.filter((b) => b.name.toLowerCase().includes(query));
  });

  const currentHeadBranch = createMemo(() => {
    return branches()?.find((b) => b.is_head)?.name ?? "HEAD";
  });

  async function handleCheckout(name: string) {
    setErrorMessage(null);
    setActionNotice(null);
    try {
      const outcome: CheckoutOutcome = await checkoutBranch(props.root, name);
      if (outcome.status === "Success") {
        setActionNotice(`Switched to branch '${name}'.`);
        refetchBranches();
        props.onCheckoutSuccess?.(name);
      } else if (outcome.status === "Conflict") {
        setCheckoutConflict({
          target: outcome.target,
          conflicting_files: outcome.conflicting_files,
          message: outcome.message,
        });
      }
    } catch (err) {
      setErrorMessage(String(err));
    }
  }

  async function handleDelete(name: string, force = false) {
    setErrorMessage(null);
    try {
      const outcome: DeleteBranchOutcome = await deleteBranch(props.root, name, force);
      if (outcome.status === "Deleted") {
        setActionNotice(`Branch '${name}' deleted.`);
        setUnmergedGuard(null);
        refetchBranches();
      } else if (outcome.status === "UnmergedGuard") {
        setUnmergedGuard({
          branch: outcome.branch,
          tip_commit: outcome.tip_commit,
          commits: outcome.commits,
          recovery_hint: outcome.recovery_hint,
        });
      }
    } catch (err) {
      setErrorMessage(String(err));
    }
  }

  return (
    <div class="branches-view">
      <div class="branches-toolbar">
        <input
          type="text"
          class="branches-filter-input"
          placeholder="Filter branches..."
          value={filter()}
          onInput={(e) => setFilter(e.currentTarget.value)}
        />
        <button class="branches-btn branches-btn-primary" onClick={() => setShowCreateModal(true)}>
          New branch
        </button>
        <button
          class="branches-btn"
          onClick={() => {
            const remotes = branches()?.filter((b) => b.is_remote);
            if (remotes && remotes.length > 0) {
              setCreateTrackingTarget(remotes[0].name);
            } else {
              setErrorMessage("No remote branches found to track.");
            }
          }}
        >
          Track remote branch
        </button>
        <button
          class="branches-btn"
          onClick={() => {
            const head = currentHeadBranch();
            const firstOther = branches()?.find((b) => b.name !== head && !b.is_remote)?.name ?? "";
            setMergeTarget(firstOther);
          }}
        >
          Merge into current...
        </button>
        <button
          class="branches-btn"
          onClick={() => {
            const head = currentHeadBranch();
            const firstOther = branches()?.find((b) => b.name !== head)?.name ?? head;
            setCompareTarget({ base: head, target: firstOther });
          }}
        >
          Compare branches
        </button>
        <button class="branches-btn" onClick={() => refetchBranches()}>
          Refresh
        </button>
      </div>

      <Show when={errorMessage()}>
        <div class="branch-unmerged-banner">{errorMessage()}</div>
      </Show>

      <Show when={actionNotice()}>
        <div class="branch-conflict-banner" style={{ "border-color": "var(--color-success)", color: "var(--color-success)", background: "var(--color-success-bg)" }}>
          {actionNotice()}
        </div>
      </Show>

      <div class="branches-group">
        <div class="branches-group-title">
          <span>Local Branches</span>
          <span class="branches-count-pill">{localBranches().length}</span>
        </div>
        <ul class="branches-list">
          <For each={localBranches()} fallback={<li class="text-muted">No local branches found.</li>}>
            {(branch) => {
              const upstreamInfo = createMemo(() => upstreamBasisLabel(branch.upstream_basis));
              return (
                <li class={`branch-item ${branch.is_head ? "branch-item-current" : ""}`}>
                  <div class="branch-item-main">
                    <span class={`branch-item-name ${branch.is_head ? "branch-item-name-current" : ""}`}>
                      {branch.name}
                    </span>
                    <Show when={branch.is_head}>
                      <span class="branch-badge-head">HEAD</span>
                    </Show>
                    <Show when={upstreamInfo()}>
                      {(info) => (
                        <Show
                          when={info().inferred}
                          fallback={
                            <span class="branch-badge-upstream">
                              {branch.ahead_behind ? (
                                <span style={{ display: "inline-flex", "align-items": "center", gap: "2px" }}>
                                  <Icon name="arrow-up" size={10} />
                                  <span>{branch.ahead_behind[0]}</span>
                                  <Icon name="arrow-down" size={10} />
                                  <span>{branch.ahead_behind[1]}</span>
                                  <span>vs {info().label}</span>
                                </span>
                              ) : (
                                info().label
                              )}
                            </span>
                          }
                        >
                          <span
                            class="branch-badge-inferred"
                            title="Inferred basis: no explicit @{u} configured, resolved to matching default remote branch"
                          >
                            {branch.ahead_behind ? (
                              <span style={{ display: "inline-flex", "align-items": "center", gap: "2px" }}>
                                <Icon name="arrow-up" size={10} />
                                <span>{branch.ahead_behind[0]}</span>
                                <Icon name="arrow-down" size={10} />
                                <span>{branch.ahead_behind[1]}</span>
                                <span>vs {info().label} (inferred)</span>
                              </span>
                            ) : (
                              `${info().label} (inferred)`
                            )}
                          </span>
                        </Show>
                      )}
                    </Show>
                    <span class="branch-item-commit">{branch.target_commit}</span>
                    <span class="branch-item-subject" title={branch.commit_subject}>
                      {branch.commit_subject}
                    </span>
                  </div>

                  <div class="branch-item-actions">
                    <Show when={!branch.is_head}>
                      <button
                        class="branches-btn branches-btn-primary"
                        onClick={() => void handleCheckout(branch.name)}
                      >
                        Checkout
                      </button>
                      <button
                        class="branches-btn"
                        onClick={() => setMergeTarget(branch.name)}
                        title={`Merge ${branch.name} into ${currentHeadBranch()}`}
                      >
                        Merge
                      </button>
                    </Show>
                    <button
                      class="branches-btn"
                      onClick={() =>
                        setCompareTarget({
                          base: currentHeadBranch(),
                          target: branch.name,
                        })
                      }
                    >
                      Compare
                    </button>
                    <button class="branches-btn" onClick={() => setRenameTarget(branch)}>
                      Rename
                    </button>
                    <Show when={props.onOpenReflog}>
                      <button
                        class="branches-btn"
                        onClick={() => props.onOpenReflog?.(branch.name)}
                        title={`View reflog for ${branch.name}`}
                      >
                        Reflog
                      </button>
                    </Show>
                    <button
                      class="branches-btn branches-btn-danger"
                      disabled={branch.is_head}
                      title={branch.is_head ? "Cannot delete active HEAD branch" : "Delete branch"}
                      onClick={() => void handleDelete(branch.name, false)}
                    >
                      Delete
                    </button>
                  </div>
                </li>
              );
            }}
          </For>
        </ul>
      </div>

      <div class="branches-group">
        <div class="branches-group-title">
          <span>Remote Branches</span>
          <span class="branches-count-pill">{remoteBranches().length}</span>
        </div>
        <ul class="branches-list">
          <For each={remoteBranches()} fallback={<li class="text-muted">No remote branches found.</li>}>
            {(branch) => (
              <li class="branch-item">
                <div class="branch-item-main">
                  <span class="branch-item-name">{branch.name}</span>
                  <span class="branch-item-commit">{branch.target_commit}</span>
                  <span class="branch-item-subject" title={branch.commit_subject}>
                    {branch.commit_subject}
                  </span>
                </div>
                <div class="branch-item-actions">
                  <button
                    class="branches-btn branches-btn-primary"
                    onClick={() => setCreateTrackingTarget(branch.name)}
                  >
                    Track in new local branch...
                  </button>
                  <button
                    class="branches-btn"
                    onClick={() => setMergeTarget(branch.name)}
                    title={`Merge ${branch.name} into ${currentHeadBranch()}`}
                  >
                    Merge
                  </button>
                  <button
                    class="branches-btn"
                    onClick={() =>
                      setCompareTarget({
                        base: currentHeadBranch(),
                        target: branch.name,
                      })
                    }
                  >
                    Compare
                  </button>
                </div>
              </li>
            )}
          </For>
        </ul>
      </div>

      {/* Task 4.2 Checkout Conflict Guard Modal */}
      <Show when={checkoutConflict()}>
        {(conflict) => (
          <div class="branch-modal-overlay">
            <div class="branch-modal-card">
              <div class="branch-modal-header">
                <h3>Checkout Conflict</h3>
                <button class="branches-btn" onClick={() => setCheckoutConflict(null)} aria-label="Close">
                  <Icon name="close" size={14} />
                </button>
              </div>
              <div class="branch-modal-body">
                <div class="branch-conflict-banner">
                  <strong>Uncommitted changes conflict with target branch '{conflict().target}'.</strong>
                  <p>
                    Git cannot switch branches because your local uncommitted changes to the following file(s) would
                    be overwritten:
                  </p>
                </div>
                <ul class="branch-file-list">
                  <For each={conflict().conflicting_files}>
                    {(file) => <li class="branch-file-item">{file}</li>}
                  </For>
                </ul>
                <p class="text-muted">
                  To protect your work, GitTree never discards uncommitted changes. You can stash your changes safely
                  and proceed, commit them first, or cancel.
                </p>
              </div>
              <div class="branch-modal-footer">
                <button
                  class="branches-btn branches-btn-primary"
                  onClick={async () => {
                    const target = conflict().target;
                    try {
                      await stashAndCheckout(props.root, target);
                      setActionNotice(`Safely stashed local changes and switched to '${target}'.`);
                      setCheckoutConflict(null);
                      refetchBranches();
                      props.onCheckoutSuccess?.(target);
                    } catch (e) {
                      setErrorMessage(String(e));
                    }
                  }}
                >
                  Stash & Checkout
                </button>
                <button
                  class="branches-btn"
                  onClick={() => {
                    setCheckoutConflict(null);
                    props.onNavigateWorkingCopy?.();
                  }}
                >
                  Commit Changes
                </button>
                <button class="branches-btn" onClick={() => setCheckoutConflict(null)}>
                  Cancel
                </button>
              </div>
            </div>
          </div>
        )}
      </Show>

      {/* Task 4.3 Unmerged-Branch Deletion Guard Modal */}
      <Show when={unmergedGuard()}>
        {(guard) => (
          <div class="branch-modal-overlay">
            <div class="branch-modal-card">
              <div class="branch-modal-header">
                <h3>Unmerged Branch Warning: '{guard().branch}'</h3>
                <button class="branches-btn" onClick={() => setUnmergedGuard(null)} aria-label="Close">
                  <Icon name="close" size={14} />
                </button>
              </div>
              <div class="branch-modal-body">
                <div class="branch-unmerged-banner">
                  <strong>Branch contains {guard().commits.length} commit(s) not merged into HEAD.</strong>
                  <p>
                    Deleting this branch will make the following commit(s) unreachable from any current branch pointer:
                  </p>
                </div>
                <ul class="branch-file-list">
                  <For each={guard().commits}>
                    {(c) => (
                      <li class="branch-file-item">
                        <strong style={{ color: "var(--color-accent)" }}>{c.sha.slice(0, 7)}</strong> — {c.subject} (
                        {c.author_name})
                      </li>
                    )}
                  </For>
                </ul>
                <div>
                  <strong>Recovery instructions:</strong>
                  <p class="text-muted">
                    If you delete this branch and later need to recover these commits, you can check the Git reflog or
                    recreate the branch from the commit SHA:
                  </p>
                  <div class="branch-recovery-box">
                    git branch {guard().branch} {guard().tip_commit}
                  </div>
                </div>
              </div>
              <div class="branch-modal-footer">
                <button
                  class="branches-btn branches-btn-danger"
                  onClick={() => void handleDelete(guard().branch, true)}
                >
                  Force Delete
                </button>
                <button class="branches-btn" onClick={() => setUnmergedGuard(null)}>
                  Cancel
                </button>
              </div>
            </div>
          </div>
        )}
      </Show>

      {/* Create Branch Modal */}
      <Show when={showCreateModal()}>
        <CreateBranchDialog
          root={props.root}
          currentHead={currentHeadBranch()}
          onClose={() => setShowCreateModal(false)}
          onSuccess={(name, checkedOut) => {
            setShowCreateModal(false);
            refetchBranches();
            if (checkedOut) props.onCheckoutSuccess?.(name);
          }}
          onConflict={(conflict) => {
            setShowCreateModal(false);
            setCheckoutConflict(conflict);
          }}
        />
      </Show>

      {/* Create Tracking Branch Modal */}
      <Show when={createTrackingTarget()}>
        {(remoteRef) => (
          <CreateTrackingBranchDialog
            root={props.root}
            remoteRef={remoteRef()}
            onClose={() => setCreateTrackingTarget(null)}
            onSuccess={(name, checkedOut) => {
              setCreateTrackingTarget(null);
              refetchBranches();
              if (checkedOut) props.onCheckoutSuccess?.(name);
            }}
          />
        )}
      </Show>

      {/* Rename Branch Modal */}
      <Show when={renameTarget()}>
        {(target) => (
          <RenameBranchDialog
            root={props.root}
            branch={target()}
            onClose={() => setRenameTarget(null)}
            onSuccess={() => {
              setRenameTarget(null);
              refetchBranches();
            }}
          />
        )}
      </Show>

      {/* Compare Branches Modal */}
      <Show when={compareTarget()}>
        {(target) => (
          <CompareBranchesDialog
            root={props.root}
            base={target().base}
            target={target().target}
            allBranches={branches()?.map((b) => b.name) ?? []}
            onClose={() => setCompareTarget(null)}
          />
        )}
      </Show>

      {/* Merge Modal */}
      <Show when={mergeTarget() !== null}>
        <MergeModal
          root={props.root}
          defaultTarget={mergeTarget()!}
          onClose={() => setMergeTarget(null)}
          onSuccess={(_newHead, isFf) => {
            setMergeTarget(null);
            refetchBranches();
            setActionNotice(
              isFf
                ? "Fast-forward merge completed."
                : "Merge commit created successfully."
            );
            setTimeout(() => setActionNotice(null), 5000);
          }}
          onConflict={(conflicts) => {
            setMergeTarget(null);
            refetchBranches();
            setErrorMessage(
              `Merge encountered conflicts in ${conflicts.length} file(s). Resolve conflicts in your working copy.`
            );
          }}
          onNavigateWorkingCopy={props.onNavigateWorkingCopy}
        />
      </Show>
    </div>
  );
};

const CreateBranchDialog: Component<{
  root: string;
  currentHead: string;
  onClose: () => void;
  onSuccess: (name: string, checkedOut: boolean) => void;
  onConflict: (conflict: { target: string; conflicting_files: string[]; message: string }) => void;
}> = (props) => {
  const [name, setName] = createSignal("");
  const [startPoint, setStartPoint] = createSignal("");
  const [checkout, setCheckout] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

  async function handleCreate() {
    const branchName = name().trim();
    if (!branchName) {
      setError("Branch name cannot be empty.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await createBranch(
        props.root,
        branchName,
        startPoint().trim() || null,
        checkout()
      );
      props.onSuccess(branchName, checkout());
    } catch (err) {
      const errStr = String(err);
      if (errStr.includes("would be overwritten by checkout")) {
        props.onConflict({
          target: branchName,
          conflicting_files: [branchName],
          message: errStr,
        });
      } else {
        setError(errStr);
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="branch-modal-overlay">
      <div class="branch-modal-card">
        <div class="branch-modal-header">
          <h3>Create New Branch</h3>
          <button class="branches-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="branch-modal-body">
          <Show when={error()}>
            <div class="branch-unmerged-banner">{error()}</div>
          </Show>
          <div class="branch-modal-field">
            <label for="new-branch-name">Branch name</label>
            <input
              id="new-branch-name"
              type="text"
              placeholder="e.g. feature/my-feature"
              value={name()}
              onInput={(e) => setName(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleCreate();
              }}
            />
          </div>
          <div class="branch-modal-field">
            <label for="start-point">Starting commit or branch (optional, defaults to HEAD)</label>
            <input
              id="start-point"
              type="text"
              placeholder={`HEAD (${props.currentHead})`}
              value={startPoint()}
              onInput={(e) => setStartPoint(e.currentTarget.value)}
            />
          </div>
          <label class="branch-modal-checkbox">
            <input
              type="checkbox"
              checked={checkout()}
              onChange={(e) => setCheckout(e.currentTarget.checked)}
            />
            <span>Checkout new branch immediately</span>
          </label>
        </div>
        <div class="branch-modal-footer">
          <button
            class="branches-btn branches-btn-primary"
            disabled={loading() || !name().trim()}
            onClick={() => void handleCreate()}
          >
            Create Branch
          </button>
          <button class="branches-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

const CreateTrackingBranchDialog: Component<{
  root: string;
  remoteRef: string;
  onClose: () => void;
  onSuccess: (name: string, checkedOut: boolean) => void;
}> = (props) => {
  const defaultLocalName = () => {
    const parts = props.remoteRef.split("/");
    return parts.length > 1 ? parts.slice(1).join("/") : props.remoteRef;
  };

  const [name, setName] = createSignal(defaultLocalName());
  const [checkout, setCheckout] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

  async function handleCreate() {
    const branchName = name().trim();
    if (!branchName) {
      setError("Local branch name cannot be empty.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await createTrackingBranch(props.root, branchName, props.remoteRef, checkout());
      props.onSuccess(branchName, checkout());
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="branch-modal-overlay">
      <div class="branch-modal-card">
        <div class="branch-modal-header">
          <h3>Create Local Tracking Branch</h3>
          <button class="branches-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="branch-modal-body">
          <Show when={error()}>
            <div class="branch-unmerged-banner">{error()}</div>
          </Show>
          <div class="branch-modal-field">
            <label>Remote Branch</label>
            <input type="text" value={props.remoteRef} disabled />
          </div>
          <div class="branch-modal-field">
            <label for="tracking-branch-name">New Local Branch Name</label>
            <input
              id="tracking-branch-name"
              type="text"
              value={name()}
              onInput={(e) => setName(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleCreate();
              }}
            />
          </div>
          <label class="branch-modal-checkbox">
            <input
              type="checkbox"
              checked={checkout()}
              onChange={(e) => setCheckout(e.currentTarget.checked)}
            />
            <span>Checkout new branch immediately</span>
          </label>
        </div>
        <div class="branch-modal-footer">
          <button
            class="branches-btn branches-btn-primary"
            disabled={loading() || !name().trim()}
            onClick={() => void handleCreate()}
          >
            Create Tracking Branch
          </button>
          <button class="branches-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

const RenameBranchDialog: Component<{
  root: string;
  branch: BranchEntry;
  onClose: () => void;
  onSuccess: () => void;
}> = (props) => {
  const [newName, setNewName] = createSignal(props.branch.name);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

  async function handleRename() {
    const val = newName().trim();
    if (!val || val === props.branch.name) {
      setError("Please specify a different branch name.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await renameBranch(props.root, props.branch.name, val);
      props.onSuccess();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class="branch-modal-overlay">
      <div class="branch-modal-card">
        <div class="branch-modal-header">
          <h3>Rename Branch '{props.branch.name}'</h3>
          <button class="branches-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="branch-modal-body">
          <Show when={error()}>
            <div class="branch-unmerged-banner">{error()}</div>
          </Show>
          <div class="branch-modal-field">
            <label for="rename-branch-name">New branch name</label>
            <input
              id="rename-branch-name"
              type="text"
              value={newName()}
              onInput={(e) => setNewName(e.currentTarget.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void handleRename();
              }}
            />
          </div>
        </div>
        <div class="branch-modal-footer">
          <button
            class="branches-btn branches-btn-primary"
            disabled={loading() || !newName().trim() || newName().trim() === props.branch.name}
            onClick={() => void handleRename()}
          >
            Rename Branch
          </button>
          <button class="branches-btn" onClick={props.onClose}>
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
};

const CompareBranchesDialog: Component<{
  root: string;
  base: string;
  target: string;
  allBranches: string[];
  onClose: () => void;
}> = (props) => {
  const [baseBranch, setBaseBranch] = createSignal(props.base);
  const [targetBranch, setTargetBranch] = createSignal(props.target);
  const [activeTab, setActiveTab] = createSignal<"ahead" | "behind" | "files">("ahead");

  const [comparison] = createResource(
    () => ({ base: baseBranch(), target: targetBranch() }),
    ({ base, target }) => compareBranches(props.root, base, target)
  );

  function swap() {
    const b = baseBranch();
    const t = targetBranch();
    setBaseBranch(t);
    setTargetBranch(b);
  }

  return (
    <div class="branch-modal-overlay">
      <div class="branch-modal-card" style={{ "max-width": "720px" }}>
        <div class="branch-modal-header">
          <h3>Branch Comparison</h3>
          <button class="branches-btn" onClick={props.onClose} aria-label="Close">
            <Icon name="close" size={14} />
          </button>
        </div>
        <div class="branch-modal-body">
          <div class="branch-compare-header">
            <div class="branch-modal-field" style={{ flex: 1 }}>
              <label>Base branch</label>
              <select value={baseBranch()} onChange={(e) => setBaseBranch(e.currentTarget.value)}>
                <For each={props.allBranches}>{(b) => <option value={b}>{b}</option>}</For>
              </select>
            </div>
            <button class="branches-btn" onClick={swap} title="Swap base and target">
              ⇄
            </button>
            <div class="branch-modal-field" style={{ flex: 1 }}>
              <label>Compare target</label>
              <select value={targetBranch()} onChange={(e) => setTargetBranch(e.currentTarget.value)}>
                <For each={props.allBranches}>{(b) => <option value={b}>{b}</option>}</For>
              </select>
            </div>
          </div>

          <Show when={comparison.loading}>
            <div class="text-muted">Comparing branches...</div>
          </Show>

          <Show when={comparison.error}>
            <div class="branch-unmerged-banner">{String(comparison.error)}</div>
          </Show>

          <Show when={comparison()}>
            {(cmp) => (
              <>
                <div class="branch-compare-summary">
                  <span>
                    <strong>'{cmp().target}'</strong> is
                  </span>
                  <span class="branch-compare-badge-ahead">+{cmp().ahead} commits ahead</span>
                  <span>and</span>
                  <span class="branch-compare-badge-behind">-{cmp().behind} commits behind</span>
                  <span>
                    <strong>'{cmp().base}'</strong>
                  </span>
                </div>

                <div class="branch-compare-tabs">
                  <button
                    class={`branch-compare-tab ${activeTab() === "ahead" ? "branch-compare-tab-active" : ""}`}
                    onClick={() => setActiveTab("ahead")}
                  >
                    Commits ahead ({cmp().ahead_commits.length})
                  </button>
                  <button
                    class={`branch-compare-tab ${activeTab() === "behind" ? "branch-compare-tab-active" : ""}`}
                    onClick={() => setActiveTab("behind")}
                  >
                    Commits behind ({cmp().behind_commits.length})
                  </button>
                  <button
                    class={`branch-compare-tab ${activeTab() === "files" ? "branch-compare-tab-active" : ""}`}
                    onClick={() => setActiveTab("files")}
                  >
                    Files changed ({cmp().changed_files.length})
                  </button>
                </div>

                <Show when={activeTab() === "ahead"}>
                  <ul class="branch-file-list" style={{ "max-height": "240px" }}>
                    <For
                      each={cmp().ahead_commits}
                      fallback={<li class="text-muted">No unique commits in target.</li>}
                    >
                      {(c) => (
                        <li class="branch-file-item">
                          <strong style={{ color: "var(--color-accent)" }}>{c.sha.slice(0, 7)}</strong> — {c.subject} (
                          {c.author_name})
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>

                <Show when={activeTab() === "behind"}>
                  <ul class="branch-file-list" style={{ "max-height": "240px" }}>
                    <For
                      each={cmp().behind_commits}
                      fallback={<li class="text-muted">No missing commits in target.</li>}
                    >
                      {(c) => (
                        <li class="branch-file-item">
                          <strong style={{ color: "var(--color-danger)" }}>{c.sha.slice(0, 7)}</strong> — {c.subject} (
                          {c.author_name})
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>

                <Show when={activeTab() === "files"}>
                  <ul class="branch-file-list" style={{ "max-height": "240px" }}>
                    <For
                      each={cmp().changed_files}
                      fallback={<li class="text-muted">No file differences.</li>}
                    >
                      {(f) => (
                        <li class="branch-file-item">
                          <span
                            class="branch-badge-upstream"
                            style={{
                              color:
                                f.status === "A"
                                  ? "var(--color-success)"
                                  : f.status === "D"
                                  ? "var(--color-danger)"
                                  : "var(--color-warning)",
                            }}
                          >
                            {f.status}
                          </span>
                          <span>{f.path}</span>
                        </li>
                      )}
                    </For>
                  </ul>
                </Show>
              </>
            )}
          </Show>
        </div>
        <div class="branch-modal-footer">
          <button class="branches-btn" onClick={props.onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
