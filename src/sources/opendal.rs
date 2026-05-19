//! OpenDAL-backed range reader implementation.

use std::io::{Error, ErrorKind};

/// Reads byte ranges from an OpenDAL operator path.
#[derive(Clone, Debug)]
pub struct Reader {
    operator: opendal::Operator,
    path: String,
    len: usize,
}

impl Reader {
    /// Creates a reader for `path` using `operator`.
    pub async fn new(operator: opendal::Operator, path: impl Into<String>) -> std::io::Result<Self> {
        /// Ensure this OpenDAL service has the minimum required capabilities
        /// to be compatible with SparseIO.
        let capabilities = operator.info().full_capability();
        if !capabilities.stat {
            return Err(Error::new(ErrorKind::InvalidInput, "OpenDAL operator does not support stat"));
        }
        if !capabilities.read {
            return Err(Error::new(ErrorKind::InvalidInput, "OpenDAL operator does not support read"));
        }

        let path = normalize_relative_path(&path.into());
        let len = operator.stat(&path).await.map_err(map_opendal_error)?.content_length();
        let len = usize::try_from(len)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "OpenDAL object length exceeds usize"))?;

        Ok(Self { operator, path, len })
    }

    /// Returns the OpenDAL path used by this reader.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl crate::Reader for Reader {
    async fn read_at(&self, offset: usize, buffer: &mut [u8]) -> std::io::Result<usize> {
        if buffer.is_empty() || offset >= self.len {
            return Ok(0);
        }

        let end = offset
            .checked_add(buffer.len())
            .map(|end| end.min(self.len))
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "range end overflow"))?;
        if end <= offset {
            return Ok(0);
        }

        let start = u64::try_from(offset).map_err(|_| Error::new(ErrorKind::InvalidInput, "offset exceeds u64"))?;
        let end = u64::try_from(end).map_err(|_| Error::new(ErrorKind::InvalidInput, "range end exceeds u64"))?;
        let data = match self.operator.read_with(&self.path).range(start..end).await {
            Ok(data) => data.to_bytes(),
            Err(err) if err.kind() == opendal::ErrorKind::RangeNotSatisfied => return Ok(0),
            Err(err) => return Err(map_opendal_error(err)),
        };

        let copied = data.len().min(buffer.len());
        buffer[..copied].copy_from_slice(&data[..copied]);
        Ok(copied)
    }

    async fn len(&self) -> std::io::Result<usize> {
        Ok(self.len)
    }
}

fn normalize_relative_path(path: &str) -> String {
    let normalized = path
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/");
    if normalized.is_empty() {
        "/".to_owned()
    } else {
        normalized
    }
}

fn map_opendal_error(err: opendal::Error) -> Error {
    err.into()
}
