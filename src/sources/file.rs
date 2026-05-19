//! Local file reader and filesystem-backed cache writer implementation.

use std::io::{ErrorKind, SeekFrom};
use std::path::{Path, PathBuf};

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

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
    async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> std::io::Result<usize> {
        let mut file = tokio::fs::File::open(&self.path).await?;
        file.seek(SeekFrom::Start(offset as u64)).await?;

        let mut total_read = 0usize;
        while total_read < buffer.len() {
            let read_len = file.read(&mut buffer[total_read..]).await?;
            if read_len == 0 {
                break;
            }
            total_read += read_len;
        }

        Ok(total_read)
    }

    async fn len(&self) -> std::io::Result<usize> {
        let size = tokio::fs::metadata(&self.path).await?.len();
        usize::try_from(size)
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "file length exceeds usize"))
    }
}

#[derive(Debug, Default)]
pub struct Writer {
    dst: PathBuf,
}

impl Writer {
    /// Creates a cache writer rooted at `dst`.
    pub fn new(dst: impl Into<PathBuf>) -> Self {
        Self { dst: dst.into() }
    }

    /// Returns the destination directory used by this writer.
    pub fn path(&self) -> &Path {
        &self.dst
    }

    fn key_path(&self, key: &str) -> PathBuf {
        self.dst.join(hex::encode(key.as_bytes()))
    }
}

impl crate::Writer for Writer {
    async fn set_cache(&mut self, key: &str, value: &[u8]) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.dst).await?;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(self.key_path(key))
            .await?;
        file.write_all(value).await?;
        file.flush().await?;
        Ok(())
    }

    async fn get_cache(&self, key: &str) -> std::io::Result<Option<Bytes>> {
        let mut file = match tokio::fs::File::open(self.key_path(key)).await {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        let mut data = Vec::new();
        file.read_to_end(&mut data).await?;
        Ok(Some(Bytes::from(data)))
    }

    async fn delete_cache(&mut self, key: &str) -> std::io::Result<()> {
        match tokio::fs::remove_file(self.key_path(key)).await {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err),
        }
    }
}
