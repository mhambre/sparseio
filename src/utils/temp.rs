use std::path::{Path, PathBuf};

/// Creates a temporary directory helper for file-backed tests and examples.
///
/// The returned directory is removed automatically when dropped.
pub fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir should be created")
}

/// Returns a child path under a temporary directory.
///
/// This keeps file-backed tests concise while still using per-test paths.
pub fn temp_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name)
}
