//! File-backed source implementations.
//!
//! This module provides two small building blocks for file-based workflows:
//! [`FileReader`], which implements [`crate::SourceReader`] for reading byte
//! ranges from a local file, and [`SparseFile`], which implements
//! [`crate::ExtentStore`] while materializing logical extents into a sparse
//! destination file.

use std::collections::BTreeMap;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};

use tokio::fs as tokio_fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::{Extent, ExtentStore, SourceReader};

/// Reads byte ranges from a local file using Tokio file I/O.
///
/// `FileReader` is intended to be used as the `SourceReader` for
/// [`crate::SparseIO`]. Each `read_at` call opens the file, seeks to the
/// requested offset, fills the provided buffer, and returns the number of bytes
/// written into it.
#[derive(Clone, Debug)]
pub struct FileReader {
    path: PathBuf,
}

impl FileReader {
    /// Creates a new reader for the file at `path`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Returns the file path used by this reader.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SourceReader for FileReader {
    async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> std::io::Result<usize> {
        let mut file = tokio_fs::File::open(&self.path).await?;
        file.seek(SeekFrom::Start(offset as u64)).await?;
        let read_len = file.read(buffer).await?;
        Ok(read_len)
    }

    async fn len(&self) -> usize {
        tokio_fs::metadata(&self.path)
            .await
            .map(|metadata| metadata.len() as usize)
            .unwrap_or(0)
    }
}

/// Stores logical extents in a sparse destination file.
///
/// The file itself is used to reserve the logical address space. Extent metadata
/// is tracked in-memory so the current `ExtentStore` trait can answer
/// `read_extent` requests. This makes the type useful as a simple example or
/// local-materialization target, but it does not currently persist extent
/// metadata across process restarts.
#[derive(Default)]
pub struct SparseFile {
    dst: PathBuf,
    extents: BTreeMap<usize, Extent>,
}

impl SparseFile {
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
}

impl ExtentStore for SparseFile {
    async fn create_extent(&mut self, offset: usize, data: bytes::Bytes) -> std::io::Result<()> {
        let length = data.len();
        let file_len = offset
            .checked_add(length)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "extent exceeds usize"))?;

        let mut file = tokio_fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&self.dst)
            .await?;

        file.seek(SeekFrom::Start(offset as u64)).await?;
        file.write_all(data.as_ref()).await?;
        file.flush().await?;
        self.extents.insert(offset, Extent { offset, length });

        Ok(())
    }

    async fn read_extent(&self, offset: usize) -> std::io::Result<Option<Extent>> {
        Ok(self.extents.get(&offset).map(|extent| Extent {
            offset: extent.offset,
            length: extent.length,
        }))
    }

    async fn delete_extent(&mut self, offset: usize) -> std::io::Result<()> {
        self.extents.remove(&offset);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use std::fs;
    use std::io::{Read, Seek, Write};
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(target_os = "linux")]
    use std::os::unix::fs::MetadataExt;
    #[cfg(target_os = "macos")]
    use std::process::Command;

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
    fn allocated_bytes(path: &Path) -> std::io::Result<u64> {
        Ok(fs::metadata(path)?.blocks() * 512)
    }

    #[cfg(target_os = "macos")]
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
    async fn file_reader_reads_requested_range() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("reader")?;
        fs::write(&path, b"hello sparseio")?;

        let reader = FileReader::new(path.clone());
        let mut buffer = [0u8; 8];
        let bytes_read = reader.read_at(6, &mut buffer).await?;

        assert_eq!(bytes_read, 8);
        assert_eq!(&buffer, b"sparseio");
        assert_eq!(reader.len().await, 14);
        assert_eq!(reader.path(), path.as_path());
        Ok(())
    }

    #[tokio::test]
    async fn file_reader_truncates_to_remaining_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("reader-tail")?;
        fs::write(&path, b"hello sparseio")?;

        let reader = FileReader::new(path);
        let mut buffer = [0xFFu8; 8];
        let bytes_read = reader.read_at(11, &mut buffer).await?;

        assert_eq!(bytes_read, 3);
        assert_eq!(&buffer[..3], b"eio");
        assert_eq!(&buffer[3..], &[0xFF; 5]);
        Ok(())
    }

    #[tokio::test]
    async fn file_reader_reports_zero_bytes_at_eof() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("reader-eof")?;
        fs::write(&path, b"hello sparseio")?;

        let reader = FileReader::new(path);
        let mut buffer = [0xAAu8; 4];
        let bytes_read = reader.read_at(14, &mut buffer).await?;

        assert_eq!(bytes_read, 0);
        assert_eq!(buffer, [0xAA; 4]);
        Ok(())
    }

    #[tokio::test]
    async fn sparse_file_tracks_created_extents() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("extent-store")?;
        let mut store = SparseFile::new(path.clone());

        store.create_extent(4096, Bytes::from_static(b"hole data")).await?;

        let extent = store.read_extent(4096).await?.expect("extent should exist");
        assert_eq!(extent.offset, 4096);
        assert_eq!(extent.length, 9);
        assert_eq!(fs::metadata(&path)?.len(), 4105);
        assert_eq!(store.path(), path.as_path());

        let mut persisted = vec![0u8; 9];
        let mut file = fs::File::open(&path)?;
        file.seek(SeekFrom::Start(4096))?;
        file.read_exact(&mut persisted)?;
        assert_eq!(&persisted, b"hole data");

        store.delete_extent(4096).await?;
        assert!(store.read_extent(4096).await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn sparse_file_preserves_zero_filled_gap_between_extents() -> Result<(), Box<dyn std::error::Error>> {
        let path = test_file_path("extent-gap")?;
        let mut store = SparseFile::new(path.clone());

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
    async fn sparse_file_uses_less_disk_than_dense_file() -> Result<(), Box<dyn std::error::Error>> {
        let sparse_path = test_file_path("sparse")?;
        let dense_path = test_file_path("dense")?;
        let mut store = SparseFile::new(sparse_path.clone());

        let logical_len = 1024 * 1024 * 1024;
        let head_len = 4 * 1024;
        let tail_len = 4 * 1024;
        let start = Bytes::from(vec![0xAB; head_len]);
        let end = Bytes::from(vec![0xCD; tail_len]);

        store.create_extent(0, start).await?;
        store.create_extent(logical_len - tail_len, end).await?;

        let logical_size = fs::metadata(&sparse_path)?.len();
        assert_eq!(logical_size, logical_len as u64);

        let mut file = fs::File::open(&sparse_path)?;

        let mut persisted_start = vec![0u8; head_len];
        file.read_exact(&mut persisted_start)?;
        assert_eq!(persisted_start, vec![0xAB; head_len]);

        let mut hole_byte = [0xFFu8; 1];
        file.seek(SeekFrom::Start((head_len + 4096) as u64))?;
        file.read_exact(&mut hole_byte)?;
        assert_eq!(hole_byte, [0]);

        let mut persisted_end = vec![0u8; tail_len];
        file.seek(SeekFrom::Start((logical_len - tail_len) as u64))?;
        file.read_exact(&mut persisted_end)?;
        assert_eq!(persisted_end, vec![0xCD; tail_len]);

        write_dense_control_file(&dense_path, logical_len)?;

        let sparse_bytes = allocated_bytes(&sparse_path)?;
        let dense_bytes = allocated_bytes(&dense_path)?;

        assert!(
            sparse_bytes < dense_bytes / 8,
            "expected sparse file to use much less disk on macOS: sparse={sparse_bytes}B dense={dense_bytes}B"
        );

        Ok(())
    }
}
