# AGENTS.md

## Project Workflow Rules

- For any new feature or larger bug fix, run `cargo check -q` before committing.
- After that successful check, create a commit in the same session.
- Do not create commits for very small tweaks/minor touch-ups only.

## Commit Scope

- Keep commits focused and descriptive.
- Include related code, UI/options wiring, and shader/render changes together when they are part of one feature/fix.
- Structural refactors should be committed crate-by-crate or subsystem-by-subsystem, not mixed with unrelated behavior changes.

## Structure Rules

- Prefer modules around 200-500 lines.
- Treat 800 lines as a hard refactor threshold for production code.
- Keep `lib.rs` and `main.rs` focused on module declarations, plugin wiring, re-exports, and startup orchestration.
- When a file crosses 800 lines, split by responsibility before adding more behavior unless there is a documented reason not to.
- For large refactors, preserve public APIs through re-exports so callers do not need broad churn.
- When changing settings or plugin wiring, keep data types, UI, persistence, and runtime registration aligned in the same change.

## Notes for Future Sessions

- Prefer validating behavior with runtime checks when rendering/input/culling logic changes.
- Keep options/settings changes wired end-to-end:
  - runtime settings struct
  - UI controls
  - options save/load persistence
