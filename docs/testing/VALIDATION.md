# Trait Validation

The `validators` feature exposes executor-neutral conformance workloads for custom
[`Reader`](../architecture/API.md#reader),
[`Writer`](../architecture/API.md#writer), and
[`Metadata`](../architecture/API.md#metadata) implementations. Each validator operates
directly on one supplied backend so failures remain attributable to that trait.

```rust
use sparseio::validators::ReaderValidator;

async fn validate(reader: MyReader) -> sparseio::Result<()> {
    let report = ReaderValidator::new(reader).validate().await?;
    println!("{report}");
    Ok(())
}
```

Validators return a `ValidationReport` naming every completed deterministic case. They
perform no automatic retries and run entirely on the caller's executor.

## Workloads

`ReaderValidator` checks stable length, empty and exact reads, repeated and overlapping
ranges, tail boundaries, and concurrent identical reads. Its configurable sample size
avoids requiring a complete download of large remote objects.

`WriterValidator` checks missing reads, exact round trips, replacement, empty values,
key isolation, concurrent independent operations, and idempotent deletion. It uses a
randomized per-run namespace and removes all values it creates.

`MetadataValidator` checks missing and exact values, replacement, empty values, prefix
scans, compare-exchange insertion and mismatch reporting, a contended atomic update,
and deletion. It also isolates and cleans up its namespace.

## Scope

A passing report establishes the documented observable contract for the exercised
workload. It cannot reproduce every distributed partition, durability failure,
permission change, or service-specific consistency mode. Backend authors should also
run native fault, load, and longevity tests in a representative environment.

Instrumented `CountingReader`, `CountingWriter`, and `CountingMetadata` implementations
are separately available through the [`testing` feature](./FEATURE.md) for integration
tests that need operation-level assertions.
