//! Post-run verification of the recording file.
//!
//! FFmpeg exiting successfully is necessary but not sufficient. This module
//! checks that the output exists, is a regular file, and contains data. It is
//! a sanity check, not a decode test: codec and duration probing with ffprobe
//! are deferred to a later milestone.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Details of a verified output file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputInfo {
    /// Size in bytes.
    pub size: u64,
}

/// Why the recording output could not be accepted.
#[derive(Debug)]
pub enum OutputError {
    /// No file exists at the configured path.
    Missing { path: PathBuf },
    /// The path exists but its metadata could not be read.
    Metadata { path: PathBuf, source: io::Error },
    /// The path exists but is not a regular file.
    NotAFile { path: PathBuf },
    /// FFmpeg reported success but wrote an empty file.
    Empty { path: PathBuf },
}

impl OutputError {
    /// Exit code for runtime failures (see README).
    pub const EXIT_CODE: u8 = 1;
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(
                f,
                "ffmpeg reported success but no output file exists: {}",
                path.display()
            ),
            Self::Metadata { path, source } => {
                write!(
                    f,
                    "could not inspect output file {}: {source}",
                    path.display()
                )
            }
            Self::NotAFile { path } => {
                write!(f, "output path is not a regular file: {}", path.display())
            }
            Self::Empty { path } => write!(
                f,
                "ffmpeg reported success but wrote no data to {}",
                path.display()
            ),
        }
    }
}

impl Error for OutputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Metadata { source, .. } => Some(source),
            Self::Missing { .. } | Self::NotAFile { .. } | Self::Empty { .. } => None,
        }
    }
}

/// Verify the file FFmpeg was asked to write.
pub fn verify(path: &Path) -> Result<OutputInfo, OutputError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Err(OutputError::Missing {
                path: path.to_path_buf(),
            });
        }
        Err(source) => {
            return Err(OutputError::Metadata {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    if !metadata.is_file() {
        return Err(OutputError::NotAFile {
            path: path.to_path_buf(),
        });
    }

    let size = metadata.len();
    if size == 0 {
        return Err(OutputError::Empty {
            path: path.to_path_buf(),
        });
    }

    Ok(OutputInfo { size })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "rust-recorder-output-{}-{label}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temporary test directory");
        dir
    }

    #[test]
    fn accepts_a_non_empty_file() {
        let dir = temp_dir("ok");
        let path = dir.join("out.mp4");
        fs::write(&path, b"data").expect("write test file");

        let info = verify(&path).expect("non-empty file is valid");
        assert_eq!(info.size, 4);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_a_missing_file() {
        let dir = temp_dir("missing");
        let path = dir.join("out.mp4");

        let error = verify(&path).expect_err("missing file must be rejected");
        assert!(matches!(error, OutputError::Missing { .. }));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_an_empty_file() {
        let dir = temp_dir("empty");
        let path = dir.join("out.mp4");
        fs::write(&path, b"").expect("write empty test file");

        let error = verify(&path).expect_err("empty file must be rejected");
        assert!(matches!(error, OutputError::Empty { .. }));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_a_directory() {
        let dir = temp_dir("dir");
        let path = dir.join("out.mp4");
        fs::create_dir(&path).expect("create directory at output path");

        let error = verify(&path).expect_err("directory must be rejected");
        assert!(matches!(error, OutputError::NotAFile { .. }));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn runtime_exit_code_is_one() {
        assert_eq!(OutputError::EXIT_CODE, 1);
    }
}
