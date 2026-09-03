//! The repository and submodule state model. Every UI surface reads from
//! records built here; no UI surface queries `git` directly (design D3).

mod resolved;
mod status;
mod submodule;
mod upstream;

pub use resolved::Resolved;
pub use status::{query_repository_state, InProgressOperation, RepositoryState};
pub use submodule::{query_submodule_matrix, GitlinkDivergence, SubmoduleState};
pub use upstream::{resolve_upstream_basis, UpstreamBasis};
