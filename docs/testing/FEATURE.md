# `testing` Feature

The `testing` feature exposes deterministic support under `sparseio::testing` without
adding hooks or synchronization to normal production builds.

- `CountingReader` and `CountingReaderFactory` serve fixed bytes and record length and
  range operations.
- `CountingWriter` and `CountingMetadata` provide small in-memory backends with
  operation counts and targeted failure injection.
- `OperationGate` pauses a chosen call at an explicit synchronization boundary for
  reproducible race tests.
- `DiskFault` injects path-scoped disk failures for the reference writer.

These utilities are intended for backend and coordinator contract tests. Keep
production dependencies on SparseIO's normal features instead.
