use std::io;

use bytes::Bytes;

use crate::Writer;

pub(crate) struct StubWriter;

impl Writer for StubWriter {
    fn write(&self, _key: &str) -> io::Result<()> {
        unimplemented!("stub writer write should not be called")
    }

    fn read(&self, _key: &str) -> io::Result<Option<Bytes>> {
        unimplemented!("stub writer read should not be called")
    }

    fn delete(&self, _key: &str) -> io::Result<()> {
        unimplemented!("stub writer delete should not be called")
    }
}
