# platform-and-packaging Specification

## Purpose
TBD - created by archiving change build-gittree-client. Update Purpose after archive.

## Requirements

### Requirement: Git invocation contract
All Git access SHALL go through a single process layer that invokes the system `git` binary with an explicitly controlled environment, passes machine-readable output flags where available, uses `-z`-delimited output for anything path-bearing, and never interpolates user input into a shell string. The layer MUST verify at startup that `git` is present and at least version 2.43, and MUST refuse to start rather than degrade silently if it is not.

#### Scenario: No shell interpolation
- **WHEN** any Git command is executed with a branch or path argument containing shell metacharacters
- **THEN** the argument reaches Git intact and no shell expansion occurs

#### Scenario: Unsupported Git version
- **WHEN** the system `git` is older than the required minimum
- **THEN** the app reports the found and required versions and does not start

#### Scenario: Environment is controlled
- **WHEN** any Git command runs
- **THEN** locale and pager variables are pinned so output parsing cannot be changed by the user's shell configuration

### Requirement: Concurrency and process discipline
Git invocations SHALL run in a bounded worker pool with a configurable limit defaulting to 8, MUST enforce per-operation timeouts, MUST terminate child processes on cancellation or app exit, and MUST NOT run two write operations against the same repository concurrently.

#### Scenario: No orphaned processes
- **WHEN** the app exits while network operations are in flight
- **THEN** all child processes are terminated and no orphan remains

#### Scenario: Writes are serialized per repository
- **WHEN** two write operations targeting the same repository are requested
- **THEN** the second waits for the first, and neither runs against a partially written index

#### Scenario: Read concurrency is not serialized
- **WHEN** many read-only queries across different submodules are requested
- **THEN** they run concurrently up to the pool limit

### Requirement: Settings persistence
Settings SHALL be stored in a documented plain-text file under the XDG config directory, be hand-editable, be validated on load with a specific error naming the invalid key, and fall back per-key to defaults rather than rejecting the whole file.

#### Scenario: Invalid single setting
- **WHEN** one setting has an invalid value
- **THEN** that key falls back to its default with a reported warning, and every other setting still applies

### Requirement: Offline and no-telemetry guarantee
The app SHALL perform no network request the user did not initiate. It MUST ship no telemetry, analytics, crash reporting or update check that transmits data, and MUST be fully functional with no network access apart from operations that inherently require a remote.

#### Scenario: Nothing is sent at startup
- **WHEN** the app starts with network monitoring in place
- **THEN** no outbound request is made

#### Scenario: Offline is fully usable
- **WHEN** the app runs with no network connectivity
- **THEN** every local capability works, and remote operations fail with a clear network error rather than hanging or degrading the UI

### Requirement: Performance budgets are enforced
The performance budgets stated across this change SHALL be asserted by an automated benchmark suite run in CI against generated fixture repositories, and a regression beyond its budget MUST fail the build.

#### Scenario: Fixtures are generated, not committed
- **WHEN** the benchmark suite runs on a clean checkout
- **THEN** it generates its fixtures — including a 25-submodule superproject and a 100 000-commit history — from a committed generator, and no large binary fixture is stored in the repository

#### Scenario: Budget regression fails CI
- **WHEN** a change pushes local submodule refresh past its 1.5 s budget
- **THEN** CI fails and names the budget, the measurement and the delta

### Requirement: Linux packaging
The app SHALL be distributed as an AppImage and an Arch package, both launching on Wayland and X11 without manual configuration. The packaged binary MUST stay under 40 MB and idle memory under 250 MB with the reference superproject open.

#### Scenario: Runs on Wayland and X11
- **WHEN** the AppImage is launched under a Wayland compositor and again under X11
- **THEN** it starts and renders correctly in both without user configuration

#### Scenario: Size and memory budgets hold
- **WHEN** a release artifact is produced
- **THEN** its size and its idle memory with the reference superproject open are measured, recorded, and fail the release if over budget

### Requirement: No repository mutation without intent
The app SHALL treat every Git write as an explicit user action. No background task, refresh, watcher, scan or view navigation may create, move or delete a ref, modify the index, modify the working tree, or run a Git command that writes — with the sole exception of updating remote-tracking refs and objects during a user-initiated fetch.

#### Scenario: Refresh is read-only
- **WHEN** any automatic refresh runs
- **THEN** an audit of the executed Git commands shows only read operations

#### Scenario: Write commands are auditable
- **WHEN** the user inspects the operation log
- **THEN** every Git command the app has executed is listed with its arguments, exit status and timestamp
