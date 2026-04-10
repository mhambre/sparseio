# `file_to_file` Example

`examples/file_to_file.rs` demonstrates sparse, out-of-order materialization from a source filesystem file into a destination filesystem file using `sparseio`.
It uses a storage-agnostic orchestrator in `examples/common/sparse_materialization.rs` and passes file-specific callbacks.

## What It Does

1. Parses CLI flags for source/destination paths and sparse materialization behavior.
2. Optionally generates a deterministic source file (`--generate-source`).
3. Builds `SparseIO` with `file::Reader` (an implementation of the `Reader` trait with `tokio::fs`) + `file::Writer` (an implementation of the `Writer` trait with `tokio::fs`).
4. Randomizes chunk read points and materializes only a selected percentage (`--fill-percent`).
5. Passes a per-step callback that computes file-specific diagnostics (logical size, allocated size, hole checks).
6. Verifies output:
   - Full fill: destination bytes must match source bytes.
   - Partial fill: written chunks must match source, unwritten chunks must still read as zeroes/null.

## Run It

```bash
cargo run --example file_to_file --features file -- \
  --src target/manual/file-to-file-src.bin \
  --dst target/manual/file-to-file-dst.bin \
  --source-len 8388608 \
  --chunk-size 262144 \
  --fill-percent 35 \
  --sleep-ms 0 \
  --progress-width 32
```

## Useful Flags

- `--generate-source` (`true` by default): Create deterministic source content.
- `--pre-size-dst`: Pre-allocate logical destination length before sparse writes.
- `--chunk-size`: Chunk size used by `SparseIO` and verification logic.
- `--fill-percent`: Percent of chunk offsets to materialize (random order).
- `--sleep-ms`: Delay between materialization steps.
- `--progress-width`: Width of the ASCII sparse map.
