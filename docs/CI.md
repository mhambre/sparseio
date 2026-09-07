# CI Pipelines

Ubuntu and macOS entry workflows call the same quality, unit, and integration
workflows. Windows runs the shared quality and unit jobs plus the disk writer contracts
that exercise its native atomic replacement path. Tests begin only after quality checks
pass.

The shared core feature set enables test support, the in-memory metadata backend, disk
writer, OpenDAL filesystem reader, and validators. Quality checks also compile default,
test-only, and representative OpenDAL memory, HTTP, S3, and SFTP combinations.

## Quality

The quality workflow runs pinned typo checking, nightly rustfmt in check mode, Clippy
with warnings denied, feature compile checks, the Rust 1.91 MSRV check, benchmark
compilation, and rustdoc with warnings and missing public documentation denied. OpenDAL service combinations are sampled
instead of combining every mutually independent integration into one oversized build.

## Tests and Coverage

Unit and documentation tests run separately from feature-backed integration tests.
Nextest supplies concise execution and failure output. The integration workflow also
runs the controlled API benchmark in smoke mode. `cargo-llvm-cov` permits zero uncovered production lines, functions, or
regions. Feature-gated test implementations under `src/utils/testing/` are excluded
from the production coverage denominator.

Run the complete local workflow with `just ci`. Individual build and verification
tasks are available in the `Justfile`. Use `just bench-components` for Divan
microbenchmarks, `just bench-api` for controlled workloads, and `just bench-smoke` for
benchmark correctness checks.
