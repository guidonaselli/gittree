//! The repository and submodule state model.

mod amend;
mod blame;
mod commit;
mod commit_detail;
mod diff;
mod discard;
mod graph;
mod history;
mod message_template;
mod resolve;
mod resolved;
mod staging;
mod status;
mod submodule;
mod upstream;
mod working_copy;
pub mod fixtures;

pub use amend::{amend, head_is_published};
pub use blame::{query_blame, BlameLine, BlameOptions};
pub use commit::{commit, CommitOptions};
pub use commit_detail::{query_commit_detail, CommitDetail, FileStat, SignatureState};
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
pub use message_template::{commit_message_template, CommitMessageTemplate};
pub use resolve::{init_repository, resolve_open, OpenOutcome};
pub use resolved::Resolved;
pub use staging::{stage_paths, unstage_paths};
pub use status::{query_repository_state, InProgressOperation, RepositoryState};
pub use submodule::{query_submodule_matrix, GitlinkDivergence, SubmoduleState};
pub use upstream::{resolve_upstream_basis, UpstreamBasis};
pub use working_copy::{
    query_working_copy_status, ChangeCode, ChangedEntry, ConflictEntry, SubmoduleFlags,
    WorkingCopyStatus,
};
