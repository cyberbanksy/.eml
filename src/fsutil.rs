//! Filesystem helpers that keep the email private while it is being rendered.

use std::io::Write;
use std::path::Path;

use tempfile::TempDir;

use crate::error::{Error, Result};

/// A temp dir only the current user can enter (0700 on Unix).
pub fn private_tempdir(prefix: &str) -> Result<TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    Ok(builder.tempdir()?)
}

/// Write a file readable only by the current user (0600 on Unix).
pub fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)?.write_all(data)?;
    Ok(())
}

/// Refuse to write through a symlink, so a planted link cannot redirect output.
pub fn reject_symlink(path: &Path) -> Result<()> {
    if let Ok(meta) = std::fs::symlink_metadata(path)
        && meta.file_type().is_symlink()
    {
        return Err(Error::Io(std::io::Error::other(format!(
            "refusing to write through symlink: {}",
            path.display()
        ))));
    }
    Ok(())
}

/// Write an output file, refusing symlinks at the destination.
pub fn write_output(path: &Path, data: &[u8]) -> Result<()> {
    reject_symlink(path)?;
    std::fs::write(path, data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn tempdir_and_file_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = private_tempdir("doteml-test-").unwrap();
        assert_eq!(
            std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let file = dir.path().join("f");
        write_private(&file, b"x").unwrap();
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn output_refuses_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::write(&target, b"keep").unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(write_output(&link, b"clobber").is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"keep");
    }
}
