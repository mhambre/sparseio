# Trait Validation

SparseIO allows consumers to supply custom [`Reader`](../architecture/API.md#reader),
[`Writer`](../architecture/API.md#writer), and
[`Metadata`](../architecture/API.md#metadata) implementations. The narrow trait APIs
keep the library extensible, but they cannot express every consistency, concurrency,
and failure-handling guarantee required by a production backend. Trait validators
provide a shared set of behavioral examples and repeatable workloads for checking
those guarantees during development.

The validators are public feature-gated utilities under `sparseio::validators`
(feature is called `validators`). Each validator accepts one consumer implementation
as the subject under test and supplies known-good in-memory implementations for the other traits.

## Intended API

Validation is asynchronous and runs on the executor chosen by the consumer. The
intended common API follows this shape:

```rust
use sparseio::validators::ReaderValidator;

async fn validate_reader(reader: MyReader) -> Result<(), Box<dyn std::error::Error>> {
    let report = ReaderValidator::new(reader).validate().await?;

    println!("{report}");
    Ok(())
}
```

Equivalent `WriterValidator` and `MetadataValidator` types validate the other public
traits. A successful result means every selected case met its expected data and
operation-count assertions. A failure identifies the case, observed result, expected
result, and deterministic workload configuration needed to reproduce it.

This API is a design target and may change while the validator module is implemented.

## Isolation Through Known-Good Implementations

Every validation environment contains instrumented in-memory implementations:

- `CountingReader` serves deterministic source bytes and records length and range-read
  operations.
- `CountingWriter` stores exact byte values and records reads, writes, and deletions by
  key.
- `CountingMetadata` stores exact metadata values and records reads, writes, and
  deletions by key.

These implementations are shared with unit and integration tests through the
feature-gated [testing support](./FEATURE.md), rather than being reimplemented by each
validator.

The implementation being validated replaces its matching in-memory component:

| Validator | Subject under test | Supplied implementations |
| --- | --- | --- |
| `ReaderValidator` | Consumer `Reader` | `CountingWriter` and `CountingMetadata` |
| `WriterValidator` | Consumer `Writer` | `CountingReader` and `CountingMetadata` |
| `MetadataValidator` | Consumer `Metadata` | `CountingReader` and `CountingWriter` |

This arrangement keeps failures attributable to one custom backend. For example, the
metadata suite can verify a consumer's remote key-value store while all source data
and cached chunks come from deterministic reference implementations.

Counting implementations observe calls made by SparseIO. They do not attempt to count
the internal database, network, or retry operations performed inside a consumer
backend.

## Common Validation Workloads

All validators run shared integration workloads through a real
[`SparseIO`](../architecture/OBJECTS.md#sparseio) coordinator.
The exact cases vary by trait, but the common suite covers:

- Cold reads that fetch a requested range and materialize the expected chunks.
- Warm reads that return the same bytes without unnecessary upstream work.
- Repeated and overlapping ranges across chunk boundaries.
- Concurrent requests for the same chunk and for partially overlapping chunks.
- Missing, stale, overwritten, and deleted cache or metadata entries.
- Empty values, boundary offsets, and other trait-specific edge cases.
- Injected transient failures followed by bounded retry behavior.
- Cache lifecycle transitions and fallback to the upstream source of truth.

Concurrency cases use deterministic synchronization points to align operations and
force contention. They should not depend on scheduler timing or arbitrary sleeps.
Assertions are made at explicit synchronization boundaries rather than inferring an
operation order from task scheduling. Workload seeds and inputs are included in
reports so a failure can be replayed.

## Operation Counts

Data equality alone may hide an inefficient or unsafe access pattern. The in-memory
components therefore record how often each trait method is called and which keys or
ranges were involved. Cases can assert properties such as:

- Concurrent misses for one chunk cause one upstream range fetch.
- A warm read does not fetch the same chunk from the upstream reader again.
- Successful materialization writes the expected chunk and metadata mapping.
- Failed reads do not publish metadata that claims unavailable data is cached.
- Deletion and expiry paths touch only the expected keys.
- Retryable failures remain within the workload's configured retry bound.

These counts describe calls across the SparseIO trait boundary. A backend remains free
to use internal batching, replication, locking, or retries as long as its observable
behavior satisfies the trait contract.

## Trait-Specific Expectations

### `ReaderValidator`

The reader suite samples direct ranges from the subject and compares them with ranges
returned through SparseIO. It checks stable length reporting, repeatable reads,
agreement between overlapping ranges, boundary behavior, concurrent access, and exact
byte outcomes. The counting writer and metadata store verify how reader results are
materialized and reused.

For very large or remote objects, validation may use deterministic sampled ranges
instead of reading the entire source. Passing therefore establishes consistency for
the exercised workload, not the truth of every byte in the object.

### `WriterValidator`

The writer suite checks write-read round trips, replacement behavior, missing reads,
deletion, independent keys, and concurrent operations. Integration cases verify that
the expected content-addressed chunks remain readable and that cache hits avoid
unnecessary calls to the counting reader.

The consumer must still define and document the consistency semantics its writer
provides when multiple processes access the same physical backend.

### `MetadataValidator`

The metadata suite checks set-get round trips, replacement behavior, missing values,
deletion, independent keys, and concurrent operations. Integration cases exercise
coverage mappings, cache publication, stale mappings, lifecycle state, and contention
between requests for the same object chunk.

The [`Metadata`](../architecture/API.md#metadata) trait does not provide transactions,
locks, conditional updates, versioning, or retries. The consumer must supply the coordination required by its
backend. Validation workloads aim to reveal lost updates, stale publication,
inconsistent reads, unsafe retry behavior, and similar failures, but the validator
does not choose or install a consistency mechanism. Valid implementations may use
locking or leases, versioned stored values with conditional updates, idempotent
operations, retries, or a combination of these techniques.

## What Validation Guarantees

A passing report shows that the implementation satisfied the selected deterministic
workloads and the documented observable trait behavior. The suites also serve as
executable examples for backend authors and establish a common baseline across
implementations.

Validation does not prove that a backend is correct under every production condition.
In particular, the in-process harness cannot reproduce every distributed race,
network partition, service restart, durability failure, permission error, or backend
configuration. Consumers should also run backend-native integration, fault-injection,
load, and longevity tests in an environment representative of production.

The validators are intended to make common integration and consistency errors easy to
find, explain, and reproduce before those broader tests begin.
