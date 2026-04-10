use bytes::Bytes;
use futures::future::{BoxFuture, Shared};

pub(crate) type SharedChunk = Shared<BoxFuture<'static, Result<Bytes, String>>>;

pub(crate) const DEFAULT_CHUNK_SIZE: usize = 128 * 1024; // 128 KiB
