# AGENTS.md

## Project Workflow Rules

- For any new feature or larger bug fix, run `cargo check -q` before committing.
- After that successful check, create a commit in the same session.
- Do not create commits for very small tweaks/minor touch-ups only.

## Commit Scope

- Keep commits focused and descriptive.
- Include related code, UI/options wiring, and shader/render changes together when they are part of one feature/fix.

## Notes for Future Sessions

- Prefer validating behavior with runtime checks when rendering/input/culling logic changes.
- Keep options/settings changes wired end-to-end:
  - runtime settings struct
  - UI controls
  - options save/load persistence
