# Task 2 Review Fix

## Changes

- Removed `Workspace.settings_open`, its initialization and action handler, and the duplicate secondary settings row. The toolbar's standard “编辑设置” dropdown and its check items remain.
- Removed the `OpenEditorSettings` action and `secondary-,` binding. The application shortcut list and its test now expect 10 bindings.
- Updated the settings isolation headless test to render the workspace, dispatch `ToggleSoftWrap` through the real window, and verify the setting changes from `false` to `true`. Existing test-only setters still cover the other settings; the test continues to check per-document isolation, text, revision, and dirty state.
- Removed the now-unused soft-wrap test setter.

## Verification

- Focused RED: `primary_application_shortcuts_use_secondary_modifier` failed as expected before the binding removal (`left: 11`, `right: 10`).
- Focused GREEN: shortcut test passed (1/1); settings isolation test passed (1/1).
- `cargo fmt --check` passed.
- `cargo test --all-targets --offline` passed (32/32).

Cargo reported its existing future-incompatibility notice for dependency `block v0.1.6`; tests completed successfully.
