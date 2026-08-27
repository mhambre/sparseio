# SparseIO Agent Guide

## Library Development Standards

SparseIO is a Rust library, not an application runtime. Prefer futures-oriented crates and APIs that remain usable from Tokio, smol, async-std, and other executors.

- Do not add Tokio as a normal library dependency unless the public API explicitly requires Tokio types. Asyncband should be the only executor-agnostic async runtime dependency for the library.
- Runtime-specific crates are acceptable in tests, examples, or feature-gated integrations when they do not leak into the core API.
- Use executor-neutral primitives for library behavior. For example, prefer timer futures such as `futures-timer` over `tokio::time` in core code.
- Keep async traits object-safe for registry and builder usage. The current backend traits use `async_trait` and are stored behind trait objects.
- Prefer owned `Bytes` when transferring large payloads into storage-like APIs. Avoid forcing callers or mock implementations to clone large byte buffers.

## Commenting Standards

Write comments for future maintainers, not for the compiler.

- Add brief rustdoc comments to structs, traits, public methods, and `pub(crate)` utilities that are meant to be reused across tests or modules.
- Describe behavior, constraints, or intent that is useful when changing the code later.
- Avoid comments that restate the function name or obvious Rust syntax.
- Keep comments short. One sentence is usually enough.
- Use inline comments sparingly, only when a small implementation detail would otherwise be easy to misread.
- Keep tests readable through names and assertions first; add comments only when the scenario itself is non-obvious.
- Avoid in-line context comments regarding conversations as they provide no value to the codebase. A good rule of thumb is that if it wont provide value to someone 5 commits down the road, it doesn't deserve to be a comment.
- Comments may only contain keyboard characters from the ASCII range. Avoid Unicode characters, emojis, or other symbols symbols in comments. This is a codebase, not a dissertation.

## Project Layout

The crate is organized around a small public API with implementation details split by responsibility.

- Keep public API surfaces in the top-level `src/` modules.
- Custom utility crates should be placed under `crates/` and used as dependencies in the main crate. They will be published separately to crates.io and may be used by other projects.
- Keep backend traits and shared public types separate from concrete implementations.
- Keep test helpers and low-level support code under `src/utils/`.
- Keep architecture and design context in `docs/`; update it when behavior or architecture changes materially.
- Prefer small modules with clear ownership over broad utility buckets.
- Avoid large modules that mix unrelated responsibilities. 
- The public API should be easy to navigate and understand without needing to read through large implementation files. As such it needs to remains small and focused on the public API surface necessary for users.

## Test Utilities

Test utilities should stay lightweight, deterministic, and easy to reason about.

- Prefer simple in-memory implementations over elaborate fixtures.
- Keep simulated latency and randomness deterministic.
- Use explicit zero-latency profiles in tests that need immediate completion or precise timing.
- Avoid making test helpers more capable than the scenarios they support.
- [divan](https://github.com/nvzqz/divan) is to be used for benchmarking and performance testing.

## Dependency Guidance

Keep the core dependency graph small and runtime-neutral.

- Normal dependencies should support library internals or public API requirements.
- Dev dependencies may use Tokio/tokio_test for `#[tokio::test]` and other test-only needs.
- Before adding a dependency, check whether the standard library or an existing crate dependency is sufficient.
- Avoid dependencies that force a specific executor, global runtime, or background task model into core library code.

## Verification

Run the relevant checks before handing off code changes.

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

The repository currently has rustfmt settings that may warn on stable Rust about nightly-only options. Treat formatting failures as actionable; the nightly-option warnings alone are expected.
