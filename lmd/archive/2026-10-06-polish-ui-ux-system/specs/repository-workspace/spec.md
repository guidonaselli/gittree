## ADDED Requirements

### Requirement: Native directory browsing
The UI SHALL provide a native directory picker button ("Browse...") adjacent to the repository open input. Activating the browse action MUST summon the system file chooser dialog via desktop portals and, upon folder selection, resolve and open the repository root.

#### Scenario: User chooses folder from native dialog
- **WHEN** the user clicks the "Browse..." action and selects a folder in the file dialog
- **THEN** the dialog returns the selected path and the app resolves and opens the repository workspace

#### Scenario: User cancels native folder dialog
- **WHEN** the user cancels the native folder dialog without picking a folder
- **THEN** no error is raised and the previous workspace state remains undisturbed
