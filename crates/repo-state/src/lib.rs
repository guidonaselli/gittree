//! The repository and submodule state model.

mod resolve;
mod resolved;
mod status;
mod submodule;
mod upstream;

pub use resolve::{init_repository, resolve_open, OpenOutcome};
pub use resolved::Resolved;
pub use status::{query_repository_state, InProgressOperation, RepositoryState};
pub use submodule::{query_submodule_matrix, GitlinkDivergence, SubmoduleState};
pub use upstream::{resolve_upstream_basis, UpstreamBasis};
