//! Example Sparse File-to-File Implementation
use sparseio::{SparseIOBuilder, sources::file::FileReader, sources::file::SparseFile};
use std::path::Path;

use bytes::Bytes;
use clap::Parser;

/// Simple sparse file-to-file implementation. This example demonstrates how to use
/// the sparse I/O library to implement a simple file-to-file copy operation that
/// supports sparse files. It reads from a source file and writes to a destination
/// file, while tracking coverage and in-flight operations.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input file path
    #[arg(short, long)]
    src: String,

    /// Output file path
    #[arg(short, long)]
    dst: String,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let src_path = Path::new(&args.src).to_path_buf();
    let dst_path = Path::new(&args.dst).to_path_buf();

    // Engines required to create a `SparseIO` instance. The `FileReader` is used to
    // read from the source file, and the `SparseFile` is our sparse destination object.
    let reader = FileReader::new(src_path);
    let storage = SparseFile::new(dst_path);

    let sparse_io = SparseIOBuilder::new()
        .inner(reader)
        .storage(storage)
        .build()
        .expect("Failed to build SparseIO instance");

    Ok(())
}
