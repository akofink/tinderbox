# Agent Guidance

- `CHARTER.md` is the source of truth for scope, invariants, and milestones. Do not expand v0.1 past it.
- Work on task branches in task worktrees under `~/dev/worktrees/tinderbox/<slug>/`; open a PR to `main` and merge only after required CI passes.
- Before pushing, run `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
- The simulation crate must remain free of graphics, windowing, and wall-clock dependencies. Keep headless tests GPU-independent and deterministic.
- Adapt useful engine code and lessons from Cascade, but do not add a Cascade crate or path dependency. Credit Cascade in the README.
- Keep code clear and focused on a fun, responsive sandbox. Commit signed conventional commits.
