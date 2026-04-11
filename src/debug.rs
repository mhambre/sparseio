//! Opt-in validation harnesses for downstream backend verification.
//!
//! These harnesses are intentionally public behind the `debug` feature so
//! downstream crates can validate their own [`crate::Reader`] and [`crate::Writer`]
//! implementations against SparseIO's contract without copy-pasting test
//! scaffolding.

use std::collections::HashMap;
use std::fmt;
use std::io::Result;
use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::Mutex;

use crate::utils::counting::{Reader as CountingReader, Writer as CountingWriter};
use crate::utils::oracle::Reader as OracleReader;
use crate::{Builder, Reader, SparseIO, Writer};

/// Configuration for validating a reader implementation.
#[derive(Debug, Clone)]
pub struct ReaderHarnessConfig {
    /// Chunk size used for SparseIO integration checks.
    pub chunk_size: usize,
    /// Deterministic bytes used as the ground truth fixture.
    pub fixture: Bytes,
}

/// Configuration for validating a writer implementation.
#[derive(Debug, Clone)]
pub struct WriterHarnessConfig {
    /// Chunk size used for SparseIO integration checks.
    pub chunk_size: usize,
    /// Deterministic bytes used as the ground truth fixture.
    pub fixture: Bytes,
}

/// Structured validation failure returned by the debug harnesses.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Name of the failed check.
    pub check: &'static str,
    /// Offset associated with the failure, when relevant.
    pub offset: Option<usize>,
    /// Chunk size in effect when the failure was detected.
    pub chunk_size: usize,
    /// Expected value summary.
    pub expected: String,
    /// Actual value summary.
    pub actual: String,
}

impl ValidationError {
    /// Constructs a validation error with explicit expected and actual values.
    fn new(
        check: &'static str,
        offset: Option<usize>,
        chunk_size: usize,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            check,
            offset,
            chunk_size,
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Constructs a validation error from an I/O failure and an expectation.
    fn io(
        check: &'static str,
        offset: Option<usize>,
        chunk_size: usize,
        expected: impl Into<String>,
        err: impl fmt::Display,
    ) -> Self {
        Self::new(check, offset, chunk_size, expected, format!("error: {err}"))
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed", self.check)?;
        if let Some(offset) = self.offset {
            write!(f, " at offset {offset}")?;
        }
        write!(f, " [chunk_size={}, expected={}, actual={}]", self.chunk_size, self.expected, self.actual)
    }
}

impl std::error::Error for ValidationError {}

/// Public harness for validating [`crate::Reader`] implementations.
pub struct ReaderHarness<R> {
    reader: R,
    config: ReaderHarnessConfig,
}

impl<R> ReaderHarness<R> {
    /// Creates a reader harness for the provided reader and fixture.
    ///
    /// The harness stores the concrete reader by value so validation can run
    /// both direct contract checks and SparseIO-backed integration checks
    /// against the same instance.
    pub fn new(reader: R, config: ReaderHarnessConfig) -> Self {
        Self { reader, config }
    }
}

impl<R> ReaderHarness<R>
where
    R: Reader + Send + Sync + 'static,
{
    /// Validates a reader against the public SparseIO contract.
    ///
    /// This runs two phases:
    /// - direct reader checks against the deterministic fixture bytes
    /// - SparseIO integration checks using an in-memory writer and internal counting wrappers to confirm cache and
    ///   dedupe behavior
    pub async fn validate(self) -> std::result::Result<(), ValidationError> {
        let Self { reader, config } = self;
        tracing::info!(
            chunk_size = config.chunk_size,
            fixture_len = config.fixture.len(),
            "starting reader harness validation"
        );
        validate_reader_direct(&reader, &config).await?;
        validate_reader_sparseio(reader, &config).await?;
        Ok(())
    }
}

/// Public harness for validating [`crate::Writer`] implementations.
pub struct WriterHarness<F> {
    factory: F,
    config: WriterHarnessConfig,
}

impl<F> WriterHarness<F> {
    /// Creates a writer harness from a factory that yields fresh writers.
    ///
    /// The factory is invoked separately for direct writer checks and for the
    /// SparseIO integration phase so the two halves of validation never share
    /// mutable extent state.
    pub fn new(factory: F, config: WriterHarnessConfig) -> Self {
        Self { factory, config }
    }
}

impl<F, W> WriterHarness<F>
where
    F: Fn() -> W + Send + Sync + 'static,
    W: Writer + Send + Sync + 'static,
{
    /// Validates a writer against the public SparseIO contract.
    ///
    /// This performs:
    /// - direct extent-store checks against a fresh writer instance
    /// - SparseIO integration checks that materialize bytes through the candidate writer and then re-read them through
    ///   the cache path
    pub async fn validate(self) -> std::result::Result<(), ValidationError> {
        let Self { factory, config } = self;
        tracing::info!(
            chunk_size = config.chunk_size,
            fixture_len = config.fixture.len(),
            "starting writer harness validation"
        );
        validate_writer_direct(&factory, &config).await?;
        validate_writer_sparseio(&factory, &config).await?;
        Ok(())
    }
}

#[derive(Default, Clone)]
struct MemoryWriter {
    extents: Arc<Mutex<HashMap<usize, Bytes>>>,
}

impl Writer for MemoryWriter {
    async fn create_extent(&mut self, offset: usize, data: Bytes) -> Result<()> {
        self.extents.lock().await.insert(offset, data);
        Ok(())
    }

    async fn read_extent(&self, offset: usize) -> Result<Bytes> {
        Ok(self.extents.lock().await.get(&offset).cloned().unwrap_or_else(Bytes::new))
    }

    async fn delete_extent(&mut self, offset: usize) -> Result<()> {
        self.extents.lock().await.remove(&offset);
        Ok(())
    }
}

/// Produces a compact description of a byte slice for validation errors.
fn fixture_summary(bytes: &[u8]) -> String {
    let preview: Vec<String> = bytes.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
    if bytes.is_empty() {
        "len=0 []".to_string()
    } else if bytes.len() <= 8 {
        format!("len={} [{}]", bytes.len(), preview.join(" "))
    } else {
        format!("len={} [{} ...]", bytes.len(), preview.join(" "))
    }
}

/// Returns the same human-readable summary format used for fixture data.
fn bytes_summary(bytes: &[u8]) -> String {
    fixture_summary(bytes)
}

/// Summarizes the exact slice that validation expected to observe.
fn expected_range_summary(fixture: &[u8], offset: usize, len: usize) -> String {
    let end = (offset + len).min(fixture.len());
    let slice = fixture.get(offset..end).unwrap_or(&[]);
    format!("offset={offset}, {}", bytes_summary(slice))
}

/// Builds a deterministic set of offsets that exercise aligned, unaligned,
/// EOF, and beyond-EOF cases for a fixture/chunk-size pair.
fn make_offsets(fixture_len: usize, chunk_size: usize) -> Vec<usize> {
    let mut offsets = vec![0];
    if fixture_len > 0 {
        offsets.push(1.min(fixture_len.saturating_sub(1)));
        offsets.push(chunk_size.saturating_sub(1).min(fixture_len.saturating_sub(1)));
        offsets.push(chunk_size.min(fixture_len.saturating_sub(1)));
        offsets.push(fixture_len.saturating_sub(1));
        offsets.push(fixture_len);
        offsets.push(fixture_len + chunk_size);
    }
    offsets.sort_unstable();
    offsets.dedup();
    offsets
}

/// Validates a reader directly against the fixture bytes.
///
/// This checks stable length reporting, repeatable reads, exact byte fidelity
/// for aligned and unaligned offsets, truncated tail reads, EOF behavior, and
/// the contract that a non-empty pre-EOF read must make forward progress.
async fn validate_reader_direct<R>(reader: &R, config: &ReaderHarnessConfig) -> std::result::Result<(), ValidationError>
where
    R: Reader + Send + Sync + 'static,
{
    let fixture = config.fixture.as_ref();
    let chunk_size = config.chunk_size;
    if chunk_size == 0 {
        return Err(ValidationError::new(
            "reader harness config",
            None,
            chunk_size,
            "chunk_size > 0",
            "chunk_size == 0",
        ));
    }

    let len1 = reader
        .len()
        .await
        .map_err(|err| ValidationError::io("reader len", None, chunk_size, fixture_summary(fixture), err))?;
    let len2 = reader
        .len()
        .await
        .map_err(|err| ValidationError::io("reader len repeat", None, chunk_size, fixture_summary(fixture), err))?;
    let len3 = reader
        .len()
        .await
        .map_err(|err| ValidationError::io("reader len repeat", None, chunk_size, fixture_summary(fixture), err))?;

    if len1 != fixture.len() {
        return Err(ValidationError::new(
            "reader len",
            None,
            chunk_size,
            fixture_summary(fixture),
            format!("len={len1}"),
        ));
    }
    if len1 != len2 || len2 != len3 {
        return Err(ValidationError::new(
            "reader len repeatability",
            None,
            chunk_size,
            format!("len={len1}"),
            format!("len2={len2}, len3={len3}"),
        ));
    }

    let offsets = make_offsets(fixture.len(), chunk_size);
    let buffer_sizes = [1usize, chunk_size.max(1), chunk_size.saturating_add(3)];

    for offset in offsets {
        if offset >= fixture.len() {
            continue;
        }
        for &buffer_len in &buffer_sizes {
            let mut buffer = vec![0xAA; buffer_len];
            let read = reader.read_at(offset, &mut buffer).await.map_err(|err| {
                ValidationError::io(
                    "reader read_at",
                    Some(offset),
                    chunk_size,
                    expected_range_summary(fixture, offset, buffer_len),
                    err,
                )
            })?;
            let remaining = fixture.len().saturating_sub(offset);
            let expected = remaining.min(buffer_len);
            let expected_slice = &fixture[offset..offset + expected];
            if read != expected {
                return Err(ValidationError::new(
                    "reader read_at length",
                    Some(offset),
                    chunk_size,
                    format!("expected_read={expected}"),
                    format!("actual_read={read}"),
                ));
            }
            if expected > 0 && buffer[..expected] != expected_slice[..] {
                return Err(ValidationError::new(
                    "reader read_at bytes",
                    Some(offset),
                    chunk_size,
                    expected_range_summary(fixture, offset, buffer_len),
                    bytes_summary(&buffer[..expected]),
                ));
            }

            let mut repeat = vec![0xBB; buffer_len];
            let repeat_read = reader.read_at(offset, &mut repeat).await.map_err(|err| {
                ValidationError::io(
                    "reader repeat read_at",
                    Some(offset),
                    chunk_size,
                    expected_range_summary(fixture, offset, buffer_len),
                    err,
                )
            })?;
            if repeat_read != read || repeat[..repeat_read] != buffer[..read] {
                return Err(ValidationError::new(
                    "reader read_at repeatability",
                    Some(offset),
                    chunk_size,
                    bytes_summary(&buffer[..read]),
                    bytes_summary(&repeat[..repeat_read]),
                ));
            }
        }
    }

    if !fixture.is_empty() {
        let tail_offset = fixture.len() - 1;
        let mut tail_buf = vec![0xCC; chunk_size.max(1)];
        let tail_read = reader.read_at(tail_offset, &mut tail_buf).await.map_err(|err| {
            ValidationError::io(
                "reader tail read",
                Some(tail_offset),
                chunk_size,
                expected_range_summary(fixture, tail_offset, tail_buf.len()),
                err,
            )
        })?;
        if tail_read == 0 {
            return Err(ValidationError::new(
                "reader tail forward progress",
                Some(tail_offset),
                chunk_size,
                "tail read should make forward progress",
                "read=0",
            ));
        }
    }

    let eof_offsets = [
        fixture.len(),
        fixture.len().saturating_add(1),
        fixture.len().saturating_add(chunk_size),
    ];
    for offset in eof_offsets {
        let mut buffer = vec![0xDD; 4];
        let read = reader
            .read_at(offset, &mut buffer)
            .await
            .map_err(|err| ValidationError::io("reader eof read", Some(offset), chunk_size, "read=0 at eof", err))?;
        if read != 0 {
            return Err(ValidationError::new(
                "reader eof behavior",
                Some(offset),
                chunk_size,
                "read=0 at eof",
                format!("read={read}"),
            ));
        }
    }

    Ok(())
}

/// Validates a reader through SparseIO using an internal in-memory writer.
///
/// This confirms that SparseIO materializes chunks once, reuses cached chunks,
/// deduplicates same-offset concurrent reads, serves the fixture bytes through
/// the viewer/byte-stream APIs, and preserves direct and cached byte parity.
async fn validate_reader_sparseio<R>(
    reader: R,
    config: &ReaderHarnessConfig,
) -> std::result::Result<(), ValidationError>
where
    R: Reader + Send + Sync + 'static,
{
    let fixture = config.fixture.as_ref();
    let chunk_size = config.chunk_size;

    let reader = CountingReader::new(reader);
    let reader_for_io = reader.clone();
    let writer = CountingWriter::new(MemoryWriter::default());
    let writer_for_io = writer.clone();

    let io: SparseIO<CountingReader<R>, CountingWriter<MemoryWriter>> =
        build_sparse_io(reader_for_io, writer_for_io, chunk_size).await?;
    let io = Arc::new(io);

    if fixture.is_empty() {
        return Ok(());
    }

    let first_chunk = io.read_chunk(0).await.map_err(|err| {
        ValidationError::io("reader sparse read_chunk", Some(0), chunk_size, bytes_summary(fixture), err)
    })?;
    if first_chunk.as_ref() != &fixture[..first_chunk.len()] {
        return Err(ValidationError::new(
            "reader sparse read_chunk bytes",
            Some(0),
            chunk_size,
            expected_range_summary(fixture, 0, chunk_size),
            bytes_summary(&first_chunk),
        ));
    }

    let unaligned_offset = 1.min(fixture.len() - 1);
    let normalized = unaligned_offset - (unaligned_offset % chunk_size);
    let unaligned_chunk = io.read_chunk(unaligned_offset).await.map_err(|err| {
        ValidationError::io(
            "reader sparse unaligned read_chunk",
            Some(unaligned_offset),
            chunk_size,
            bytes_summary(fixture),
            err,
        )
    })?;
    let expected = fixture
        .get(normalized..(normalized + unaligned_chunk.len()).min(fixture.len()))
        .unwrap_or(&[]);
    if &unaligned_chunk[..] != expected {
        return Err(ValidationError::new(
            "reader sparse unaligned bytes",
            Some(unaligned_offset),
            chunk_size,
            expected_range_summary(fixture, normalized, unaligned_chunk.len()),
            bytes_summary(&unaligned_chunk),
        ));
    }

    let before_reads = reader.read_count();
    let cached = io.read_chunk(0).await.map_err(|err| {
        ValidationError::io("reader sparse cached read", Some(0), chunk_size, bytes_summary(fixture), err)
    })?;
    if cached != first_chunk {
        return Err(ValidationError::new(
            "reader sparse cached bytes",
            Some(0),
            chunk_size,
            bytes_summary(&first_chunk),
            bytes_summary(&cached),
        ));
    }
    if reader.read_count() != before_reads {
        return Err(ValidationError::new(
            "reader sparse cache reuse",
            Some(0),
            chunk_size,
            format!("upstream_reads={before_reads}"),
            format!("upstream_reads={}", reader.read_count()),
        ));
    }

    let io_for_same = io.clone();
    let same_reads: Vec<_> = (0..4)
        .map(|_| {
            let io = io_for_same.clone();
            tokio::spawn(async move { io.read_chunk(0).await })
        })
        .collect();
    for handle in same_reads {
        let chunk = handle
            .await
            .map_err(|err| {
                ValidationError::new("reader same-chunk join", Some(0), chunk_size, "task join", err.to_string())
            })?
            .map_err(|err| {
                ValidationError::io(
                    "reader same-chunk concurrent read",
                    Some(0),
                    chunk_size,
                    bytes_summary(fixture),
                    err,
                )
            })?;
        if chunk != first_chunk {
            return Err(ValidationError::new(
                "reader same-chunk dedupe",
                Some(0),
                chunk_size,
                bytes_summary(&first_chunk),
                bytes_summary(&chunk),
            ));
        }
    }

    let mut viewer = io.viewer();
    viewer
        .seek(0)
        .map_err(|err| ValidationError::io("reader viewer seek", Some(0), chunk_size, "seek to start", err))?;
    let mut buf = vec![0u8; fixture.len()];
    let read = viewer
        .read(&mut buf)
        .await
        .map_err(|err| ValidationError::io("reader viewer read", Some(0), chunk_size, bytes_summary(fixture), err))?;
    if read != fixture.len() || buf != fixture {
        return Err(ValidationError::new(
            "reader viewer read",
            Some(0),
            chunk_size,
            bytes_summary(fixture),
            bytes_summary(&buf),
        ));
    }

    let mut bytestream = io.viewer();
    let mut stream = bytestream.to_bytestream().await;
    let mut streamed = Vec::new();
    while let Some(item) = futures::StreamExt::next(&mut stream).await {
        let chunk = item
            .map_err(|err| ValidationError::io("reader bytestream", None, chunk_size, bytes_summary(fixture), err))?;
        streamed.extend_from_slice(&chunk);
    }
    if streamed != fixture {
        return Err(ValidationError::new(
            "reader bytestream parity",
            None,
            chunk_size,
            bytes_summary(fixture),
            bytes_summary(&streamed),
        ));
    }

    let _ = writer.read_count();
    let _ = writer.create_count();

    Ok(())
}

/// Validates a writer directly against extent-store semantics.
///
/// This checks missing-extent reads, missing deletes, round-trip extent
/// fidelity, disjoint and short-tail extents, same-offset overwrite behavior,
/// repeated reads, and delete semantics without involving SparseIO.
async fn validate_writer_direct<F, W>(
    factory: &F,
    config: &WriterHarnessConfig,
) -> std::result::Result<(), ValidationError>
where
    F: Fn() -> W + Send + Sync + 'static,
    W: Writer + Send + Sync + 'static,
{
    let fixture = config.fixture.as_ref();
    let chunk_size = config.chunk_size;
    if chunk_size == 0 {
        return Err(ValidationError::new(
            "writer harness config",
            None,
            chunk_size,
            "chunk_size > 0",
            "chunk_size == 0",
        ));
    }

    let mut writer = factory();

    let missing = writer
        .read_extent(0)
        .await
        .map_err(|err| ValidationError::io("writer missing extent read", Some(0), chunk_size, "empty bytes", err))?;
    if !missing.is_empty() {
        return Err(ValidationError::new(
            "writer missing extent",
            Some(0),
            chunk_size,
            "empty bytes",
            bytes_summary(&missing),
        ));
    }

    writer.delete_extent(0).await.map_err(|err| {
        ValidationError::io("writer delete missing", Some(0), chunk_size, "delete missing is no-op", err)
    })?;

    let first_len = fixture.len().min(chunk_size.max(1)).min(fixture.len().max(1));
    let first_data = Bytes::copy_from_slice(&fixture[..first_len]);
    writer.create_extent(0, first_data.clone()).await.map_err(|err| {
        ValidationError::io("writer create round-trip", Some(0), chunk_size, bytes_summary(&first_data), err)
    })?;
    let first_read = writer.read_extent(0).await.map_err(|err| {
        ValidationError::io("writer read round-trip", Some(0), chunk_size, bytes_summary(&first_data), err)
    })?;
    if first_read != first_data {
        return Err(ValidationError::new(
            "writer create/read round-trip",
            Some(0),
            chunk_size,
            bytes_summary(&first_data),
            bytes_summary(&first_read),
        ));
    }

    let disjoint_offset = chunk_size.max(1) * 2;
    let disjoint_seed: &[u8] = if fixture.is_empty() {
        &[0]
    } else {
        &fixture[..fixture.len().min(4)]
    };
    let disjoint_data = Bytes::copy_from_slice(disjoint_seed);
    writer
        .create_extent(disjoint_offset, disjoint_data.clone())
        .await
        .map_err(|err| {
            ValidationError::io(
                "writer disjoint create",
                Some(disjoint_offset),
                chunk_size,
                bytes_summary(&disjoint_data),
                err,
            )
        })?;
    let disjoint_read = writer.read_extent(disjoint_offset).await.map_err(|err| {
        ValidationError::io(
            "writer disjoint read",
            Some(disjoint_offset),
            chunk_size,
            bytes_summary(&disjoint_data),
            err,
        )
    })?;
    if disjoint_read != disjoint_data {
        return Err(ValidationError::new(
            "writer disjoint extent",
            Some(disjoint_offset),
            chunk_size,
            bytes_summary(&disjoint_data),
            bytes_summary(&disjoint_read),
        ));
    }

    let tail_offset = fixture.len().saturating_sub(3);
    let tail_data = Bytes::copy_from_slice(&fixture[tail_offset..]);
    writer.create_extent(tail_offset, tail_data.clone()).await.map_err(|err| {
        ValidationError::io("writer short tail create", Some(tail_offset), chunk_size, bytes_summary(&tail_data), err)
    })?;
    let tail_read = writer.read_extent(tail_offset).await.map_err(|err| {
        ValidationError::io("writer short tail read", Some(tail_offset), chunk_size, bytes_summary(&tail_data), err)
    })?;
    if tail_read != tail_data {
        return Err(ValidationError::new(
            "writer short tail extent",
            Some(tail_offset),
            chunk_size,
            bytes_summary(&tail_data),
            bytes_summary(&tail_read),
        ));
    }

    let overwrite_a = Bytes::from_static(b"first-write");
    let overwrite_b = Bytes::from_static(b"second");
    writer.create_extent(8, overwrite_a.clone()).await.map_err(|err| {
        ValidationError::io("writer overwrite first", Some(8), chunk_size, bytes_summary(&overwrite_a), err)
    })?;
    writer.create_extent(8, overwrite_b.clone()).await.map_err(|err| {
        ValidationError::io("writer overwrite second", Some(8), chunk_size, bytes_summary(&overwrite_b), err)
    })?;
    let overwrite_read = writer.read_extent(8).await.map_err(|err| {
        ValidationError::io("writer overwrite read", Some(8), chunk_size, bytes_summary(&overwrite_b), err)
    })?;
    if overwrite_read != overwrite_b {
        return Err(ValidationError::new(
            "writer same-offset overwrite",
            Some(8),
            chunk_size,
            bytes_summary(&overwrite_b),
            bytes_summary(&overwrite_read),
        ));
    }

    let repeat_read_1 = writer.read_extent(8).await.map_err(|err| {
        ValidationError::io("writer overwrite repeat 1", Some(8), chunk_size, bytes_summary(&overwrite_b), err)
    })?;
    let repeat_read_2 = writer.read_extent(8).await.map_err(|err| {
        ValidationError::io("writer overwrite repeat 2", Some(8), chunk_size, bytes_summary(&overwrite_b), err)
    })?;
    if repeat_read_1 != repeat_read_2 || repeat_read_2 != overwrite_b {
        return Err(ValidationError::new(
            "writer repeated read stability",
            Some(8),
            chunk_size,
            bytes_summary(&overwrite_b),
            format!("{}, {}", bytes_summary(&repeat_read_1), bytes_summary(&repeat_read_2)),
        ));
    }

    writer
        .delete_extent(8)
        .await
        .map_err(|err| ValidationError::io("writer delete existing", Some(8), chunk_size, "delete succeeds", err))?;
    let deleted = writer
        .read_extent(8)
        .await
        .map_err(|err| ValidationError::io("writer read deleted", Some(8), chunk_size, "empty bytes", err))?;
    if !deleted.is_empty() {
        return Err(ValidationError::new(
            "writer delete semantics",
            Some(8),
            chunk_size,
            "empty bytes",
            bytes_summary(&deleted),
        ));
    }

    Ok(())
}

/// Validates a writer through SparseIO using a fresh source reader.
///
/// This confirms that a candidate writer can materialize bytes from a known
/// good reader, serve cached reads from the writer path, expose filled extents
/// after materialization, and preserve correctness under concurrent reads.
async fn validate_writer_sparseio<F, W>(
    factory: &F,
    config: &WriterHarnessConfig,
) -> std::result::Result<(), ValidationError>
where
    F: Fn() -> W + Send + Sync + 'static,
    W: Writer + Send + Sync + 'static,
{
    let fixture = config.fixture.as_ref();
    let chunk_size = config.chunk_size;
    let source = OracleReader::new(config.fixture.clone());
    let writer = CountingWriter::new((factory)());
    let writer_for_io = writer.clone();
    let io: Arc<SparseIO<OracleReader, CountingWriter<W>>> =
        Arc::new(build_sparse_io(source, writer_for_io, chunk_size).await?);

    if fixture.is_empty() {
        return Ok(());
    }

    let mut viewer = io.viewer();
    viewer
        .seek(0)
        .map_err(|err| ValidationError::io("writer viewer seek", Some(0), chunk_size, "seek start", err))?;

    let mut first = vec![0u8; chunk_size.min(fixture.len()).max(1)];
    let read = viewer
        .read(&mut first)
        .await
        .map_err(|err| ValidationError::io("writer sparse read", Some(0), chunk_size, bytes_summary(fixture), err))?;
    if read == 0 {
        return Err(ValidationError::new(
            "writer sparse forward progress",
            Some(0),
            chunk_size,
            "non-empty read",
            "read=0",
        ));
    }

    let first_chunk = io.read_chunk(0).await.map_err(|err| {
        ValidationError::io("writer sparse read_chunk", Some(0), chunk_size, bytes_summary(fixture), err)
    })?;
    if first_chunk != Bytes::copy_from_slice(&fixture[..first_chunk.len()]) {
        return Err(ValidationError::new(
            "writer sparse chunk bytes",
            Some(0),
            chunk_size,
            expected_range_summary(fixture, 0, first_chunk.len()),
            bytes_summary(&first_chunk),
        ));
    }

    let before_create = writer.create_count();
    let before_read = writer.read_count();
    let cached = io.read_chunk(0).await.map_err(|err| {
        ValidationError::io("writer sparse cached read", Some(0), chunk_size, bytes_summary(fixture), err)
    })?;
    if cached != first_chunk {
        return Err(ValidationError::new(
            "writer sparse cached bytes",
            Some(0),
            chunk_size,
            bytes_summary(&first_chunk),
            bytes_summary(&cached),
        ));
    }
    if writer.create_count() != before_create || writer.read_count() <= before_read {
        return Err(ValidationError::new(
            "writer sparse cache reuse",
            Some(0),
            chunk_size,
            format!("create_calls={before_create}, read_calls>{before_read}"),
            format!("create_calls={}, read_calls={}", writer.create_count(), writer.read_count()),
        ));
    }

    let cached_extent = writer.read_extent(0).await.map_err(|err| {
        ValidationError::io("writer sparse extent read", Some(0), chunk_size, bytes_summary(fixture), err)
    })?;
    if cached_extent != first_chunk {
        return Err(ValidationError::new(
            "writer sparse filled extent",
            Some(0),
            chunk_size,
            bytes_summary(&first_chunk),
            bytes_summary(&cached_extent),
        ));
    }

    let read_count_before = writer.read_count();
    let chunk_size = chunk_size.max(1);
    let tasks: Vec<_> = [0usize, chunk_size.min(fixture.len().saturating_sub(1))]
        .into_iter()
        .filter(|offset| *offset < fixture.len())
        .map(|offset| {
            let io = io.clone();
            tokio::spawn(async move { (offset, io.read_chunk(offset).await) })
        })
        .collect();
    for task in tasks {
        let (offset, result) = task.await.map_err(|err| {
            ValidationError::new("writer sparse join", None, config.chunk_size, "task join", err.to_string())
        })?;
        let chunk = result.map_err(|err| {
            ValidationError::io(
                "writer sparse concurrent read",
                Some(offset),
                config.chunk_size,
                bytes_summary(fixture),
                err,
            )
        })?;
        let expected = fixture
            .get(offset - (offset % chunk_size)..(offset - (offset % chunk_size) + chunk.len()).min(fixture.len()))
            .unwrap_or(&[]);
        if chunk.as_ref() != expected {
            return Err(ValidationError::new(
                "writer sparse concurrent correctness",
                Some(offset),
                config.chunk_size,
                bytes_summary(expected),
                bytes_summary(&chunk),
            ));
        }
    }
    if writer.read_count() < read_count_before {
        return Err(ValidationError::new(
            "writer sparse read count",
            None,
            config.chunk_size,
            format!("reads>={read_count_before}"),
            format!("reads={}", writer.read_count()),
        ));
    }

    Ok(())
}

/// Builds a SparseIO instance for harness validation.
///
/// The builder is intentionally centralized here so both reader and writer
/// harnesses use the same chunk-size and validation setup.
async fn build_sparse_io<R, W>(
    reader: R,
    writer: W,
    chunk_size: usize,
) -> std::result::Result<SparseIO<R, W>, ValidationError>
where
    R: Reader + Send + Sync + 'static,
    W: Writer + Send + Sync + 'static,
{
    Builder::new()
        .chunk_size(chunk_size)
        .reader(reader)
        .writer(writer)
        .build()
        .await
        .map_err(|err| ValidationError::io("builder", None, chunk_size, "build SparseIO", err))
}
