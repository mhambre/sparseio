use std::path::Path;

/// Returns the on-disk allocated size of a file when supported by the host.
///
/// Linux uses `stat` metadata, macOS uses `du -k`, and other platforms return
/// `Ok(None)` because the sparse-file observation is not available there.
#[cfg(target_os = "linux")]
pub fn allocated_bytes(path: &Path) -> std::io::Result<Option<u64>> {
    use std::os::unix::fs::MetadataExt;

    Ok(Some(std::fs::metadata(path)?.blocks() * 512))
}

/// Returns the on-disk allocated size of a file when supported by the host.
#[cfg(target_os = "macos")]
pub fn allocated_bytes(path: &Path) -> std::io::Result<Option<u64>> {
    use std::process::Command;

    let output = Command::new("du").arg("-k").arg(path).output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!(
            "du -k failed for {} with status {:?}",
            path.display(),
            output.status.code()
        )));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    let kib = stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "du output missing size column"))?
        .parse::<u64>()
        .map_err(std::io::Error::other)?;

    Ok(Some(kib * 1024))
}

/// Returns `Ok(None)` on unsupported platforms.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn allocated_bytes(_path: &Path) -> std::io::Result<Option<u64>> {
    Ok(None)
}
