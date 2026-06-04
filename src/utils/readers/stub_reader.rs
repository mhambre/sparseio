use std::io;

use bytes::Bytes;

use crate::Reader;

pub(crate) struct StubReader;

impl Reader for StubReader {
    fn len(&self) -> io::Result<usize> {
        unimplemented!("stub reader len should not be called")
    }

    fn read_at(&self, _offset: usize, _length: usize) -> io::Result<Bytes> {
        unimplemented!("stub reader read_at should not be called")
    }
}
