# Benchmarks

SparseIO separates component diagnostics from user-facing API workloads.

## Component benchmarks

Run `just bench-components`, optionally with a Divan name filter such as
`just bench-components warm_read`. These microbenchmarks isolate local costs and are
useful for finding regressions; they are not production latency claims.

| Target | Question answered |
| --- | --- |
| `filesystem` | What overhead do direct OpenDAL and disk-backed SparseIO add to a raw Tokio range read? |
| `warm_read` | How much latency does the complete warm coordinator path add above `DiskWriter` and raw Tokio? |
| `coordinator_read` | How do cache hits, materialization, assembly, expiration, and stream dispatch scale? |
| `concurrency` | Does singleflight remain stable, and does configured chunk concurrency work? |
| `disk_writer` | What do validation and atomic publication cost above direct Tokio file operations? |
| `memory_metadata` | What do metadata operations and prefix scans cost under contention? |
| `registry` | What do URI resolution, construction, coordinator open, and alias registration cost? |
| `lifecycle` | How do invalidation, refresh, deletion, and permanent-cache collection scale? |

Divan byte counters appear only when the timed operation processes exactly the reported
bytes. Shared `Bytes` slices and sub-chunk reads intentionally omit throughput counters
instead of presenting impossible memory bandwidth.

## API workloads

Run `just bench-api`. The harness uses real TCP and OpenDAL against a loopback
HuggingFace-compatible service. Deterministic profiles control response latency,
jitter, bandwidth, and server concurrency:

| Profile | Base latency | Jitter | Bandwidth | Upstream concurrency | Arrival interval |
| --- | ---: | ---: | ---: | ---: | ---: |
| `lan` | 1 ms | +/- 250 us | 500 MiB/s | 16 | 250 us |
| `regional` | 20 ms | +/- 5 ms | 100 MiB/s | 16 | 2 ms |
| `wan` | 60 ms | +/- 15 ms | 25 MiB/s | 16 | 5 ms |

Use `just bench-api all` to run all profiles. The default 1,000 observations make p99
a tail measurement rather than an extrapolation from a handful of samples. Requests
are released on a fixed schedule, so a slow response does not delay the next request
and hide queueing latency. Fan-out batches start duplicate callers together.

The harness reports HDR Histogram p50, p95, p99, maximum latency, wall throughput,
failures, and observed upstream reads and bytes for:

- direct OpenDAL cold ranges;
- SparseIO cold ranges with source length loaded before timing;
- SparseIO warm-cache ranges;
- duplicate direct source fan-out;
- SparseIO singleflight fan-out.

Direct and SparseIO pairs use the same object sequence, range size, network profile,
and request schedule. Fixture construction, source stat calls, and cache warming happen
outside the timed window. Each cold object has unique content so content-addressed
deduplication cannot accidentally turn later observations into cache hits.

Pass `format=json` to `just bench-api` for machine-readable output. Run
`just bench-api-live hf://models/owner/repository/object` only for exploratory public
Hub measurements. Live results depend on routing, service load, credentials, and local
network conditions and must not be used as reproducible regression thresholds.

## Verification

`just bench-smoke` executes every Divan case once and runs a small zero-delay API
workload. CI compiles both benchmark feature sets and runs the API smoke workload on
Linux, macOS, and Windows. Benchmark builds omit `testing`, so fault injection and test
observability hooks are absent from measured library paths.

Record the compiler version, hardware, filesystem, profile, request count, fan-out, and
feature set with retained results. Compare repeated runs on the same idle host; shared
CI machines are suitable for correctness smoke tests, not latency budgets.
