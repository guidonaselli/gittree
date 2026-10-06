## ADDED Requirements

### Requirement: Vector SVG icon system
The application shell, navigation bars, submodule matrix indicators, diff viewers, and modal dialogs SHALL render vector SVG icons rather than Unicode emojis or raw text glyphs. All SVG icons MUST use currentColor or design token fill/stroke and support accessible tooltips or `aria-hidden` tags.

#### Scenario: Status indicators and modal controls render SVG icons
- **WHEN** viewing warning badges, success checks, dirty markers, search inputs, or dialog dismiss buttons
- **THEN** vector SVG icons are displayed with consistent geometric proportions across display scales

### Requirement: Settings and Preferences dialog
The application SHALL provide a Settings dialog accessible through the header action bar, the command palette, and the standard `Ctrl+,` shortcut. The settings dialog MUST provide controls to inspect and adjust theme preferences (with live palette preview), Follow Desktop synchronization, network concurrency limits, fetch timeouts, and default branch names.

#### Scenario: Open settings dialog
- **WHEN** the user presses `Ctrl+,` or triggers the Settings action
- **THEN** the Preferences modal opens, showing current XDG configuration values with tabbed sections

#### Scenario: Save settings to XDG
- **WHEN** the user updates a configuration property in the settings modal
- **THEN** the change is validated, written through the settings API, and immediately reflected across open workspace tabs
