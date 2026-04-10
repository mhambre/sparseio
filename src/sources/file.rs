//! File-backed source implementations.
//!
//! This module provides two small building blocks for file-based workflows:
//! [`Reader`], which implements [`crate::Reader`] for reading byte
//! ranges from a local file, and [`Writer`], which implements
//! [`crate::Writer`] while materializing logical extents into a sparse
//! destination file (<https://wiki.archlinux.org/title/Sparse_file>).

use std::collections::BTreeMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use bytes::Bytes;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::fd::{AsRawFd, RawFd};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::MetadataExt;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

/// Reads byte ranges from a local file using Tokio file I/O.
///
/// `Reader` is intended to be used as the [`crate::Reader`] for
/// [`crate::SparseIO`]. Each `read_at` call opens the file, seeks to the
/// requested offset, fills the provided buffer, and returns the number of bytes
/// written into it.
#[derive(Clone, Debug)]
pub struct Reader {
    path: PathBuf,
}

impl Reader {
    /// Creates a new reader for the file at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the file path used by this reader.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl crate::Reader for Reader {
    /// Reads at most `buffer.len()` bytes starting at `offset`.
    async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> std::io::Result<usize> {
        let mut file = tokio::fs::File::open(&self.path).await?;
        file.seek(SeekFrom::Start(offset as u64)).await?;
        let read_len = file.read(buffer).await?;
        Ok(read_len)
    }

    /// Returns the source file length in bytes.
    async fn len(&self) -> std::io::Result<usize> {
        let size = tokio::fs::metadata(&self.path).await?.len();
        usize::try_from(size)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "file length exceeds usize"))
    }
}

/// Stores cached file extents in a sparse destination file.
///
/// The file itself is used to reserve the logical address space. Extent metadata
/// is tracked in-memory so the current [`crate::Writer`] trait can answer `read_extent`
/// requests efficiently. This makes the type useful as a simple example or
/// local-materialization target, but it does not currently persist extent
/// metadata across process restarts.
#[derive(Default)]
pub struct Writer {
    dst: PathBuf,
    extents: BTreeMap<usize, Extent>,
}

#[derive(Clone, Copy, Debug)]
/// Internal metadata struct for tracking extents in `Writer`.
/// Each extent is defined by its starting offset and length.
struct Extent {
    offset: usize,
    length: usize,
}

impl Writer {
    /// Creates a sparse destination file store rooted at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            dst: path.into(),
            extents: BTreeMap::new(),
        }
    }

    /// Returns the destination path used by this store.
    pub fn path(&self) -> &Path {
        &self.dst
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    /// Punches out the storage backing a single extent while keeping logical size.
    async fn punch_extent(&self, extent: Extent) -> std::io::Result<()> {
        if extent.length == 0 {
            return Ok(());
        }

        let file = tokio::fs::OpenOptions::new().write(true).open(&self.dst).await?;
        let metadata = file.metadata().await?;
        let logical_len = usize::try_from(metadata.len())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "file length exceeds usize"))?;
        let end = extent.offset.saturating_add(extent.length).min(logical_len);
        let block_size = metadata.blksize().max(1) as usize;
        let fd = file.as_raw_fd();

        if let Err(err) = punch_hole_aligned(fd, extent.offset, end, block_size) {
            if is_unsupported_punch_error(&err) {
                return Ok(());
            }
            return Err(err);
        }

        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    /// No-op hole punch fallback on unsupported platforms.
    async fn punch_extent(&self, _extent: Extent) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
/// Punches a hole for `[start, end)` after filesystem block alignment.
fn punch_hole_aligned(fd: RawFd, start: usize, end: usize, block_size: usize) -> std::io::Result<()> {
    if end <= start {
        return Ok(());
    }

    let aligned_start = align_up(start, block_size);
    let aligned_end = align_down(end, block_size);
    if aligned_end <= aligned_start {
        return Ok(());
    }

    punch_hole(fd, aligned_start, aligned_end - aligned_start)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
/// Attempts hole punching and suppresses unsupported-filesystem errors.
fn punch_hole_best_effort(fd: RawFd, start: usize, end: usize, block_size: usize) -> std::io::Result<()> {
    if let Err(err) = punch_hole_aligned(fd, start, end, block_size) {
        if is_unsupported_punch_error(&err) {
            return Ok(());
        }
        return Err(err);
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
/// Aligns `value` upward to the nearest `alignment` boundary.
fn align_up(value: usize, alignment: usize) -> usize {
    if alignment <= 1 {
        return value;
    }
    let rem = value % alignment;
    if rem == 0 {
        value
    } else {
        value.saturating_add(alignment - rem)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
/// Aligns `value` downward to the nearest `alignment` boundary.
fn align_down(value: usize, alignment: usize) -> usize {
    if alignment <= 1 {
        return value;
    }
    value - (value % alignment)
}

impl crate::Writer for Writer {
    /// Writes `data` at `offset` and records the resulting extent.
    async fn create_extent(&mut self, offset: usize, data: bytes::Bytes) -> std::io::Result<()> {
        let length = data.len();
        let end = offset
            .checked_add(length)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "extent exceeds usize"))?;

        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&self.dst)
            .await?;

        file.seek(SeekFrom::Start(offset as u64)).await?;
        file.write_all(data.as_ref()).await?;
        self.extents.insert(offset, Extent { offset, length });

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            let metadata = file.metadata().await?;
            let logical_len = usize::try_from(metadata.len())
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "file length exceeds usize"))?;
            let block_size = metadata.blksize().max(1) as usize;
            let fd = file.as_raw_fd();

            // Punch only local gaps around this extent instead of scanning all extents.
            let prev_end = self
                .extents
                .range(..offset)
                .next_back()
                .map(|(_, extent)| extent.offset.saturating_add(extent.length))
                .unwrap_or(0);
            let next_start = self
                .extents
                .range(offset.saturating_add(1)..)
                .next()
                .map(|(_, extent)| extent.offset);

            if prev_end < offset {
                punch_hole_best_effort(fd, prev_end, offset, block_size)?;
            }

            if let Some(next_start) = next_start {
                if end < next_start {
                    punch_hole_best_effort(fd, end, next_start, block_size)?;
                }
            } else if end < logical_len {
                punch_hole_best_effort(fd, end, logical_len, block_size)?;
            }
        }

        Ok(())
    }

    /// Reads a previously tracked extent at `offset`, or returns empty bytes.
    async fn read_extent(&self, offset: usize) -> std::io::Result<Bytes> {
        let Some(extent) = self.extents.get(&offset).copied() else {
            return Ok(Bytes::new());
        };

        let mut file = tokio::fs::File::open(&self.dst).await?;
        file.seek(SeekFrom::Start(extent.offset as u64)).await?;

        let mut data = vec![0u8; extent.length];
        file.read_exact(&mut data).await?;

        Ok(Bytes::from(data))
    }

    /// Removes tracked extent metadata and hole-punches that extent range.
    async fn delete_extent(&mut self, offset: usize) -> std::io::Result<()> {
        if let Some(extent) = self.extents.remove(&offset) {
            self.punch_extent(extent).await?;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
/// Linux hole punching via `fallocate(FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE)`.
fn punch_hole(fd: RawFd, offset: usize, len: usize) -> std::io::Result<()> {
    if len == 0 {
        return Ok(());
    }

    let offset = libc::off_t::try_from(offset)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "offset exceeds off_t"))?;
    let len = libc::off_t::try_from(len)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "length exceeds off_t"))?;

    let result = unsafe { libc::fallocate(fd, libc::FALLOC_FL_PUNCH_HOLE | libc::FALLOC_FL_KEEP_SIZE, offset, len) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
/// macOS hole punching via `fcntl(F_PUNCHHOLE)`.
fn punch_hole(fd: RawFd, offset: usize, len: usize) -> std::io::Result<()> {
    if len == 0 {
        return Ok(());
    }

    let offset = libc::off_t::try_from(offset)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "offset exceeds off_t"))?;
    let len = libc::off_t::try_from(len)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "length exceeds off_t"))?;

    #[repr(C)]
    struct Fpunchhole {
        fp_flags: libc::c_uint,
        reserved: libc::c_uint,
        fp_offset: libc::off_t,
        fp_length: libc::off_t,
    }

    const F_PUNCHHOLE: libc::c_int = 99;
    let mut punch = Fpunchhole {
        fp_flags: 0,
        reserved: 0,
        fp_offset: offset,
        fp_length: len,
    };

    let result = unsafe { libc::fcntl(fd, F_PUNCHHOLE, &mut punch) };
    if result != -1 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
/// Returns true when a hole-punch failure indicates the filesystem does not support it.
fn is_unsupported_punch_error(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(code) if code == libc::ENOTSUP || code == libc::EOPNOTSUPP)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Reader as _, Writer as _};
    use bytes::Bytes;
    use std::fs;
    use std::io::{Read, Seek, Write};
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(target_os = "linux")]
    use std::os::unix::fs::MetadataExt;
    #[cfg(target_os = "macos")]
    use std::process::Command;

    /// Creates a unique test file path under `target/sparse-file-tests`.
    fn test_file_path(name: &str) -> std::io::Result<PathBuf> {
        let dir = Path::new("target").join("sparse-file-tests");
        fs::create_dir_all(&dir)?;

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        Ok(dir.join(format!("{name}-{unique}.dat")))
    }

    #[cfg(target_os = "linux")]
    /// Returns on-disk allocated bytes for `path` on Linux.
    fn allocated_bytes(path: &Path) -> std::io::Result<u64> {
        Ok(fs::metadata(path)?.blocks() * 512)
    }

    #[cfg(target_os = "macos")]
    /// Returns on-disk allocated bytes for `path` on macOS via `du -k`.
    fn allocated_bytes(path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
        let output = Command::new("du").arg("-k").arg(path).output()?;
        assert!(
            output.status.success(),
            "du -k failed for {} with status {:?}",
            path.display(),
            output.status.code()
        );

        let stdout = String::from_utf8(output.stdout)?;
        let kib = stdout
            .split_whitespace()
            .next()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "du output missing size column"))?
            .parse::<u64>()?;

        Ok(kib * 1024)
    }

    /// Writes a dense control file of exactly `len` bytes.
    fn write_dense_control_file(path: &Path, len: usize) -> std::io::Result<()> {
        let mut file = fs::File::create(path)?;
        let chunk = vec![0x5Au8; 1024 * 1024];
        let mut remaining = len;

        while remaining > 0 {
            let to_write = remaining.min(chunk.len());
            file.write_all(&chunk[..to_write])?;
            remaining -= to_write;
        }

        file.sync_all()?;
        Ok(())
    }

    #[tokio::test]
    /// Verifies file-backed reader returns the requested byte range.
    async fn file_reader_reads_requested_range() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("reader")?;
        fs::write(&path, b"hello sparseio")?;

        let reader = Reader::new(path.clone());
        let mut buffer = [0u8; 8];
        let bytes_read = reader.read_at(6, &mut buffer).await?;

        assert_eq!(bytes_read, 8);
        assert_eq!(&buffer, b"sparseio");
        assert_eq!(reader.len().await?, 14);
        assert_eq!(reader.path(), path.as_path());
        Ok(())
    }

    #[tokio::test]
    /// Verifies reads near EOF are truncated to remaining bytes.
    async fn file_reader_truncates_to_remaining_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("reader-tail")?;
        fs::write(&path, b"hello sparseio")?;

        let reader = Reader::new(path);
        let mut buffer = [0xFFu8; 8];
        let bytes_read = reader.read_at(11, &mut buffer).await?;

        assert_eq!(bytes_read, 3);
        assert_eq!(&buffer[..3], b"eio");
        assert_eq!(&buffer[3..], &[0xFF; 5]);
        Ok(())
    }

    #[tokio::test]
    /// Verifies reads at EOF return zero bytes.
    async fn file_reader_reports_zero_bytes_at_eof() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("reader-eof")?;
        fs::write(&path, b"hello sparseio")?;

        let reader = Reader::new(path);
        let mut buffer = [0xAAu8; 4];
        let bytes_read = reader.read_at(14, &mut buffer).await?;

        assert_eq!(bytes_read, 0);
        assert_eq!(buffer, [0xAA; 4]);
        Ok(())
    }

    #[tokio::test]
    /// Verifies extent creation, retrieval, persistence, and deletion behavior.
    async fn sparse_file_tracks_created_extents() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("extent-store")?;
        let mut store = Writer::new(path.clone());

        store.create_extent(4096, Bytes::from_static(b"hole data")).await?;

        let data = store.read_extent(4096).await?;
        assert_eq!(data, Bytes::from_static(b"hole data"));
        assert_eq!(fs::metadata(&path)?.len(), 4105);
        assert_eq!(store.path(), path.as_path());

        let mut persisted = vec![0u8; 9];
        let mut file = fs::File::open(&path)?;
        file.seek(SeekFrom::Start(4096))?;
        file.read_exact(&mut persisted)?;
        assert_eq!(&persisted, b"hole data");

        store.delete_extent(4096).await?;
        assert!(store.read_extent(4096).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    /// Verifies bytes between disjoint extents remain zero-filled.
    async fn sparse_file_preserves_zero_filled_gap_between_extents() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("extent-gap")?;
        let mut store = Writer::new(path.clone());

        let front = Bytes::from_static(b"front");
        let back = Bytes::from_static(b"back");
        let back_offset = 8192;

        store.create_extent(0, front.clone()).await?;
        store.create_extent(back_offset, back.clone()).await?;

        let mut file = fs::File::open(&path)?;

        let mut front_bytes = vec![0u8; front.len()];
        file.read_exact(&mut front_bytes)?;
        assert_eq!(front_bytes, front.as_ref());

        let gap_len = back_offset - front.len();
        let mut gap = vec![0u8; gap_len];
        file.read_exact(&mut gap)?;
        assert!(gap.iter().all(|byte| *byte == 0));

        let mut back_bytes = vec![0u8; back.len()];
        file.read_exact(&mut back_bytes)?;
        assert_eq!(back_bytes, back.as_ref());
        assert_eq!(fs::metadata(&path)?.len(), (back_offset + back.len()) as u64);

        Ok(())
    }

    #[tokio::test]
    /// Verifies sparse layout uses substantially less allocated disk than dense layout.
    async fn sparse_file_uses_less_disk_than_dense_file() -> Result<(), Box<dyn std::error::Error>> {
        let sparse_path = test_file_path("sparse")?;
        let dense_path = test_file_path("dense")?;
        let mut store = Writer::new(sparse_path.clone());

        let logical_len = 1024 * 1024 * 1024;
        let head_len = 4 * 1024;
        let tail_len = 4 * 1024;
        let start = Bytes::from(vec![0xAB; head_len]);
        let end = Bytes::from(vec![0xCD; tail_len]);

        store.create_extent(0, start).await?;
        store.create_extent(logical_len - tail_len, end).await?;

        let logical_size = fs::metadata(&sparse_path)?.len();
        assert_eq!(logical_size, logical_len as u64);

        write_dense_control_file(&dense_path, logical_len)?;

        let sparse_bytes = allocated_bytes(&sparse_path)?;
        let dense_bytes = allocated_bytes(&dense_path)?;

        assert!(
            sparse_bytes < dense_bytes / 8,
            "expected sparse file to use much less disk on macOS: sparse={sparse_bytes}B dense={dense_bytes}B"
        );

        Ok(())
    }

    #[tokio::test]
    /// Verifies deleting a missing extent is a no-op rather than an error.
    async fn delete_missing_extent_is_a_noop() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("delete-missing")?;
        let mut store = Writer::new(path);

        store.delete_extent(1024).await?;
        store.delete_extent(1024).await?;
        assert!(store.read_extent(1024).await?.is_empty());
        Ok(())
    }

    #[tokio::test]
    /// Verifies same-offset writes follow last-write-wins semantics.
    async fn same_offset_overwrite_is_last_write_wins() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("overwrite")?;
        let mut store = Writer::new(path);

        store.create_extent(2048, Bytes::from_static(b"first")).await?;
        store.create_extent(2048, Bytes::from_static(b"second")).await?;

        assert_eq!(store.read_extent(2048).await?, Bytes::from_static(b"second"));
        Ok(())
    }

    #[tokio::test]
    /// Verifies short tail extents round-trip exactly as written.
    async fn short_tail_extent_round_trips_exact_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("tail")?;
        let mut store = Writer::new(path);

        store.create_extent(4093, Bytes::from_static(b"tail")).await?;
        assert_eq!(store.read_extent(4093).await?, Bytes::from_static(b"tail"));
        Ok(())
    }

    #[tokio::test]
    /// Verifies file-open and file-path failures propagate as I/O errors.
    async fn file_open_and_path_error_propagation_is_preserved() -> Result<(), Box<dyn std::error::Error>> {
        let missing = test_file_path("missing-source")?;
        let reader = Reader::new(&missing);
        let mut buffer = [0u8; 4];
        let err = reader.read_at(0, &mut buffer).await.expect_err("missing source should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

        let dst = test_file_path("missing-parent/subdir/dst.bin")?;
        let mut store = Writer::new(&dst);
        let err = store
            .create_extent(0, Bytes::from_static(b"data"))
            .await
            .expect_err("missing parent should fail");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(store.read_extent(0).await?.is_empty());
        Ok(())
    }
}
