use std::io;

use bytes::Bytes;

use crate::Metadata;

pub(crate) struct StubMetadata;

impl Metadata for StubMetadata {
    fn get(&self, _key: &str) -> io::Result<Option<Bytes>> {
        unimplemented!("stub metadata get should not be called")
    }

    fn set(&self, _key: &str, _value: Bytes) -> io::Result<()> {
        unimplemented!("stub metadata set should not be called")
    }

    fn delete(&self, _key: &str) -> io::Result<()> {
        unimplemented!("stub metadata delete should not be called")
    }
}
