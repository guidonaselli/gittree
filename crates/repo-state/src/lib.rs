//! The repository and submodule state model.

mod amend;
mod blame;
mod branches;
mod commit;
mod commit_detail;
mod conflicts;
mod diff;
mod discard;
mod graph;
mod history;
mod ignore;
mod integration;
mod message_template;
mod resolve;
mod resolved;
mod staging;
mod stashes;
mod status;
mod submodule;
mod tags;
mod upstream;
mod working_copy;
pub mod fixtures;

pub use amend::{amend, head_is_published};
pub use blame::{query_blame, BlameLine, BlameOptions};
pub use branches::{
    checkout_branch, compare_branches, create_branch, create_tracking_branch, delete_branch,
    query_branches, rename_branch, stash_and_checkout, BranchComparison, BranchComparisonFile,
    BranchEntry, CheckoutOutcome, DeleteBranchOutcome,
};
pub use commit::{commit, CommitOptions};
pub use commit_detail::{query_commit_detail, CommitDetail, FileStat, SignatureState};
pub use conflicts::{
    check_conflict_markers_in_text, check_file_conflict_markers, launch_mergetool,
    query_conflicts, query_mergetool_config, resolve_conflict, stage_paths_with_guard,
    ConflictItem, ConflictMarkerInfo, ConflictResolution, ConflictType, MergetoolConfig,
    MergetoolOutcome, StageOutcome, SubmoduleCandidateCommit, SubmoduleConflictInfo,
};
pub use diff::{
    diff_file, diff_file_with_options, diff_revisions, read_blob_base64,
    read_working_tree_file_base64, stage_hunks, stage_lines, unstage_hunks, unstage_lines,
    DiffViewOptions, FileDiff, Hunk, NonTextualDiff,
};
pub use discard::{delete_untracked_paths, discard_tracked_paths, stash_paths};
pub use graph::{query_history_graph, GraphResult, GraphRow, LANE_BUDGET};
pub use history::{
    query_file_at_revision, query_history_count, query_history_page, search_history, CommitSummary,
    ContentSearchMode, HistoricalFile, HistoryScope, HistorySearchOptions, HistorySearchResult,
};
pub use ignore::{
    add_ignore_rule, check_ignore, compute_ignore_pattern, IgnoreExplanation, IgnorePatternKind,
    IgnoreTarget,
};
pub use integration::{
    abort_operation, check_dirty_tree, continue_operation, query_active_operation,
    query_conflicting_files, query_rebase_plan, skip_operation, start_cherry_pick, start_interactive_rebase,
    start_merge, start_revert, AbortOutcome, ActiveOperationDetail, ActiveOperationKind,
    CherryPickOptions, CherryPickOutcome, DirtyTreeDetails, MergeOptions, MergeOutcome,
    OperationStepOutcome, RebaseAction, RebaseOutcome, RebasePlanItem, RevertOptions,
    RevertOutcome,
};
pub use message_template::{commit_message_template, CommitMessageTemplate};
pub use resolve::{init_repository, resolve_open, OpenOutcome};
pub use resolved::Resolved;
pub use staging::{stage_paths, unstage_paths};
pub use stashes::{
    apply_stash, clear_stashes, create_stash, drop_stash, inspect_stash, pop_stash, query_stashes,
    CreateStashOptions, StashApplyOutcome, StashDetail, StashEntry, StashFileStat,
};
pub use status::{query_repository_state, InProgressOperation, RepositoryState};
pub use submodule::{query_submodule_matrix, GitlinkDivergence, SubmoduleState};
pub use tags::{
    create_tag, delete_remote_tag, delete_tag, push_tag, query_remote_tags, query_tags,
    CreateTagOptions, PushTagOptions, TagEntry,
};
pub use upstream::{resolve_upstream_basis, UpstreamBasis};
pub use working_copy::{
    query_working_copy_status, query_working_copy_status_cancellable, ChangeCode, ChangedEntry,
    ConflictEntry, SubmoduleFlags, WorkingCopyStatus,
};
