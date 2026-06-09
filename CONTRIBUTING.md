# Contributing

Thank you for your interest in contributing to do-board.

## Prerequisites

- Rust ≥ 1.80
- `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)
- Dioxus CLI for frontend builds (`cargo install dioxus-cli`)

## Workflow

1. Fork the repository and create a branch from `main`:
   ```sh
   git checkout -b feat/my-feature
   ```

2. Make your changes. Keep commits focused and atomic.

3. Ensure the project builds and checks pass:
   ```sh
   cargo check --workspace
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   cargo fmt --check
   ```

4. Open a pull request against `main`. Fill in the PR template.

## Commit conventions

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(widget): add transport widget
fix(backend): handle missing API key gracefully
refactor(shared): rename WidgetSize to Size
docs: update widget table in README
```

## Adding a widget

1. Add the variant to `WidgetType` in `shared/src/lib.rs`.
2. Add a backend handler in `backend/src/` to fetch the data.
3. Add the Dioxus component in `frontend/src/widgets/`.
4. Register it in the widget router in `frontend/src/main.rs`.

## Code style

- `cargo fmt` is enforced in CI.
- `cargo clippy -- -D warnings` must pass.
- No `unwrap()` in production paths — use `?` or explicit error handling.
- Comments only when the *why* is non-obvious.

## Reporting issues

Open an issue on GitHub with a clear description, steps to reproduce, and the output of `cargo --version`.
