## ADDED Requirements

### Requirement: Hierarchical tree navigation sidebar
The repository sidebar SHALL render an organized, hierarchical tree structure dividing the repository into distinct collapsible sections: Workspace, Branches, Submodules, and Stashes/Tags.

#### Scenario: Collapsing and expanding sidebar tree sections
- **WHEN** the user clicks the section header toggle of Branches or Submodules
- **THEN** the section collapses or expands smoothly without layout shift, and its collapsed state is preserved.

#### Scenario: Inline submodule quick status
- **WHEN** submodules are registered and initialized in the active repository
- **THEN** each submodule appears inline under the Submodules section showing its relative name, checked-out branch (or detached SHA), dirty indicator, and divergence status against the superproject gitlink.

#### Scenario: Inline branch listing and active HEAD indication
- **WHEN** local branches exist in the repository
- **THEN** local branches appear under the Branches section with the currently checked out branch clearly emphasized with a distinct active indicator and token-based badge.

#### Scenario: One-click submodule tab drill-in
- **WHEN** the user clicks a submodule row in the sidebar
- **THEN** the submodule opens in a tab strip item within the current workspace group, and the main view updates to the submodule's working copy.

#### Scenario: Quick branch checkout from sidebar
- **WHEN** the user double-clicks an inactive local branch in the sidebar
- **THEN** the client switches to that branch if the working tree is clean, or displays the safe checkout conflict modal if uncommitted changes would be overwritten.

#### Scenario: Responsive top sync toolbar
- **WHEN** the window width is narrowed to >= 940px
- **THEN** the sync controls (Fetch, Pull, Push) and repository tabs remain clearly visible and accessible without clipping, overlapping, or horizontal text truncation.
