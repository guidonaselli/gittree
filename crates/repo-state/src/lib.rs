//! The repository and submodule state model.

mod amend;
mod commit;
mod diff;
mod discard;
mod resolve;
mod resolved;
mod staging;
mod status;
mod submodule;
mod upstream;
mod working_copy;

pub use amend::{amend, head_is_published};
pub use commit::{commit, CommitOptions};
pub use diff::{diff_file, stage_hunks, stage_lines, unstage_hunks, unstage_lines, FileDiff, Hunk};
pub use discard::{delete_untracked_paths, discard_tracked_paths, stash_paths};
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
