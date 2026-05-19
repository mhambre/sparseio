# file_to_file Example

`examples/file_to_file.rs` demonstrates reading selected ranges from a local file while populating a filesystem file cache.

## What It Does

1. Optionally generates a deterministic local source file.
2. Builds `SparseIO` with `file::Reader` for upstream reads and `file::Writer` for cache storage.
3. Reads a few chunk-aligned offsets to populate the cache directory.
4. Reopens the same cache directory to show metadata-driven chunk-size reuse.

## Run It

```bash
cargo run --example file_to_file --features impl-file,metadata-memory -- \
  --src target/manual/file-to-file-src.bin \
  --dst target/manual/file-to-file-cache \
  --source-len 8388608 \
  --chunk-size 262144
```
