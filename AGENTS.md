# AGENTS.md

## Project overview
Rust Cargo workspace with multiple crates under `exporter/`, `receiver/` and a single one in `confui/`. The final artifact is built and deployed as a container image from the repository root.

## Layout
- `Cargo.toml`: workspace root, shared dependencies and versions
- `confui/`: WASM Configuration UI
- `exporter/`: Multiple exporter-crates, each in his own directory
- `receiver/`: Multiple receiver-crates, each in his own directory
- `shared/`: Shared code fragments used in all crates
- `Containerfile`: multi-stage build, the deployment entry point
- `config/`: local configuration files. Never add these to git, a container or a deployment.
- `doc/`: Documentation of the receivers, exporters and the configuration ui

## Commands
Run from the repository root.
- Build all: `cargo build --workspace`
- Build one crate: `cargo build -p <crate>`
- Test all: `cargo test --workspace`
- Lint: `cargo clippy --workspace --all-targets -- -D warnings`
- Format: `cargo fmt --all` (check only: `cargo fmt --all -- --check`)
- Container build: `podman build -f Containerfile -t ingesto:dev .` (or `docker build -f Containerfile -t myapp:dev .`)

## Rules
- Add new dependencies as `[workspace.dependencies.dep]` in the root `Cargo.toml`, then reference them with `dep = { workspace = true }` in crates.
- Do not add dependencies to a single crate unless there is a clear reason.
- Do not depend on `tokio` for threadding and parallel processes unless there is a clear reason.
- Do not use `unwrap()` or `expect()` outside tests. Return `Result` with `thiserror` (libraries) or `anyhow` (binaries).
- Do not commit secrets, `.env` files, or credentials.
- Do not modify `Cargo.lock` by hand.
- Always add tests, at least for Edge-Cases.

## Crate-specific instructions
Some crates have their own `AGENTS.md` with additional rules. Read it before editing files in that crate:
- `confui/AGENTS.md`: WASM Configuration UI

## Definition of done
Before reporting a task as complete, all of these must pass from the repository root:
1. `cargo fmt --all -- --check`
2. `cargo test --workspace`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `podman build -f Containerfile -t ingesto:dev .` succeeds

## Dockerfile conventions
- Keep the workspace-wide build context in the root. Use `.dockerignore` to exclude `target/`, `.git/`, `doc/` and `deploy/`.
- Always use the `Containerfile` for container builds with `podman` (or `docker`) for a release build.
- Pin base image tags. Do not use `latest`.

## Git
- Keep commits focused on one crate or concern.
- Run the definition-of-done checks before each commit.
- Never commit without asking.
- Never push changes!
