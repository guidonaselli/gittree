## ADDED Requirements

### Requirement: Design-token architecture
Every colour, spacing, radius, border, shadow, font family, font size and animation duration in the UI SHALL resolve through a named design token. No component may contain a literal colour or raw spacing value, and a build-time check MUST fail the build when one is introduced.

#### Scenario: Literal value fails the build
- **WHEN** a component is committed containing a hex colour or a raw pixel spacing value
- **THEN** the token lint fails with the file, line and offending value

#### Scenario: A theme needs no code
- **WHEN** a new theme is authored
- **THEN** it consists only of token values, and no component source changes are required

### Requirement: Built-in themes
The app SHALL ship a light and a dark theme, both meeting WCAG AA contrast for all text and for all state indicators, and MUST NOT encode meaning in colour alone.

#### Scenario: Contrast is enforced automatically
- **WHEN** the theme test suite runs
- **THEN** every foreground/background token pair used for text is asserted to meet AA, and the suite fails on any pair that does not

#### Scenario: State is not colour-only
- **WHEN** a submodule row is behind, ahead, dirty or detached
- **THEN** each state is distinguishable by shape, icon or text in addition to colour

### Requirement: User themes
The user SHALL be able to install a theme as a single plain-text file in a documented directory, select it at runtime without restarting, and see a malformed theme rejected with a specific parse error while the previous theme stays active. A missing token in a user theme MUST fall back to the built-in default for that token rather than rendering unstyled.

#### Scenario: Live theme switch
- **WHEN** the user selects another installed theme
- **THEN** the whole UI restyles without a restart and without losing view state

#### Scenario: Malformed theme is rejected safely
- **WHEN** a theme file has invalid syntax
- **THEN** the app names the file and line, keeps the current theme active, and does not crash or render unstyled

#### Scenario: Partial theme is completed
- **WHEN** a user theme defines only a subset of tokens
- **THEN** unspecified tokens inherit the built-in default of matching light or dark polarity

### Requirement: Desktop theme adoption
The app SHALL follow the desktop's light/dark preference by default. Where a supported desktop theme daemon publishes its active palette as a readable file, the app MAY read it and map its roles (accent, background levels, foreground levels, and the ANSI-style semantic colours) onto GitTree's own design tokens, live, without a restart. Adoption MUST be overridable by an explicit in-app theme choice, MUST label itself generically in the UI (as following the desktop, never naming the specific theme daemon or vendor), and a detection or parse failure MUST fall back to the built-in theme silently rather than erroring.

#### Scenario: System switches to dark
- **WHEN** the desktop switches its colour scheme while the app is running and no explicit theme is set
- **THEN** the app follows within one frame budget

#### Scenario: Desktop accent palette changes live
- **WHEN** the desktop's published theme file changes while desktop-sync is active and no explicit override is set
- **THEN** GitTree's tokens update to the new palette without a restart, within the same debounce window as other filesystem-watched changes

#### Scenario: Malformed palette file falls back silently
- **WHEN** the desktop's published theme file exists but cannot be parsed
- **THEN** the app keeps the current theme, logs the failure, and does not error or crash

#### Scenario: Explicit choice wins
- **WHEN** the user has selected a specific theme
- **THEN** desktop preference or desktop-sync changes do not override it

### Requirement: Application layout
The app SHALL present a repository sidebar, a primary content area and a contextual detail panel; panes SHALL be resizable and collapsible; and layout, pane sizes and per-repository active view MUST persist across restarts.

#### Scenario: Layout survives restart
- **WHEN** the user resizes panes, collapses the sidebar and relaunches
- **THEN** the same layout is restored

#### Scenario: Minimum viewport
- **WHEN** the window is 1280×720
- **THEN** no primary control is clipped or unreachable

### Requirement: Group and tab navigation
Each open repository SHALL be a workspace group with its own tab strip; a group switcher SHALL let the user pick which group's tabs are currently shown, and only the active group's tab strip is visible at a time. Switching groups or tabs MUST be a pure UI operation: it MUST NOT refetch data that is already loaded and unchanged, and MUST be reachable from the keyboard.

#### Scenario: Group switcher lists every open repository
- **WHEN** three repositories are open at once
- **THEN** the group switcher names all three, marks the active one, and switching is a single action

#### Scenario: Tab strip shows only the active group's tabs
- **WHEN** group A has two submodule tabs open and group B has none
- **THEN** switching to group B shows only its own (empty) tab strip, not group A's tabs

#### Scenario: Keyboard tab cycling
- **WHEN** the user invokes the next/previous-tab shortcut
- **THEN** the active tab within the current group advances or retreats without a pointer

### Requirement: Keyboard operability
Every action in the app SHALL be reachable and executable from the keyboard. Focus order MUST be logical, the focused element MUST always be visibly indicated, and no modal may trap focus without an Escape path.

#### Scenario: Full staging and commit without a mouse
- **WHEN** the user stages a hunk, writes a message and commits using only the keyboard
- **THEN** every step is reachable, and focus is visible at each step

#### Scenario: Keybindings are rebindable
- **WHEN** the user rebinds a shortcut to one already in use
- **THEN** the conflict is reported with the other action named, and the change is not applied silently

### Requirement: Command palette
The app SHALL provide a command palette that searches every available action and every repository, states each action's keybinding, shows why an unavailable action is unavailable, and never executes a destructive action directly from a single palette selection without its normal confirmation.

#### Scenario: Unavailable action explains itself
- **WHEN** the user finds an action that cannot run in the current state
- **THEN** the palette shows it as unavailable with the reason, rather than hiding it or failing on selection

#### Scenario: Destructive action still confirms
- **WHEN** a destructive action is invoked from the palette
- **THEN** its normal confirmation with blast radius is shown

### Requirement: Error presentation
Any failed Git operation SHALL surface the command's own exit status and verbatim stderr, in addition to any friendlier summary. The app MUST NOT replace a Git error with a generic message, and MUST NOT report success when the underlying command failed.

#### Scenario: Raw output is always available
- **WHEN** an operation fails
- **THEN** the verbatim command line and stderr are viewable and copyable

#### Scenario: No false success
- **WHEN** a Git command exits non-zero
- **THEN** the operation is reported as failed regardless of any partial output parsed

### Requirement: Accessibility
Interactive elements SHALL expose accessible names and roles, respect the system's reduced-motion preference, and remain usable at 200% text scaling.

#### Scenario: Reduced motion is honoured
- **WHEN** the system requests reduced motion
- **THEN** non-essential animation and transitions are disabled
