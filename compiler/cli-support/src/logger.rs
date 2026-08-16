//! Provides configuration of a logger.
//!
//! Both command line programs accept the same `--verbose`/`--log-file`
//! options and want the same logger behavior from them. They differ only in
//! how they report a failure to the user, so [`configure`] returns
//! [`LogError`] and each program maps that onto its own error type.

use env_logger::Builder;
use log::trace;
use log::LevelFilter;
use std::fmt;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

/// The highest verbosity level that the command line programs accept.
const MAX_VERBOSITY: u8 = 4;

/// A failure to configure the logger.
///
/// The [`fmt::Display`] text is the default message for each failure. A
/// command line program that wants different wording matches on the variant
/// instead.
#[derive(Debug)]
pub enum LogError {
    /// The requested verbosity is greater than [`MAX_VERBOSITY`].
    VerbosityOutOfRange(u8),
    /// The file named by `--log-file` could not be created.
    LogFileCreate(PathBuf, std::io::Error),
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogError::VerbosityOutOfRange(_) => {
                write!(f, "Verbosity level must be between 0 and {MAX_VERBOSITY}")
            }
            LogError::LogFileCreate(path, err) => {
                write!(f, "Unable to create log file {}. {}", path.display(), err)
            }
        }
    }
}

/// Configures the log with the specified verbosity.
///
/// Higher verbosity results in additional log messages
/// up to a maximum verbosity level.
///
/// This installs the process-wide logger and therefore succeeds at most once
/// in the lifetime of a program.
pub fn configure(verbosity: u8, log_file: Option<PathBuf>) -> Result<(), LogError> {
    builder(verbosity, log_file.as_deref())?.init();

    Ok(())
}

/// Builds the logger for the specified verbosity without installing it.
///
/// Separated from [`configure`] so that the mapping from verbosity to level,
/// and the log file handling, are testable. Installing the logger is
/// process-wide and can only happen once, so it cannot be part of what tests
/// exercise repeatedly.
fn builder(verbosity: u8, log_file: Option<&Path>) -> Result<Builder, LogError> {
    let log_level = match verbosity {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        4 => LevelFilter::Trace,
        _ => return Err(LogError::VerbosityOutOfRange(verbosity)),
    };

    trace!("Logger verbosity {log_level}");

    let mut builder = Builder::new();

    if let Some(log_location) = log_file {
        let file = File::create(log_location)
            .map_err(|e| LogError::LogFileCreate(log_location.to_path_buf(), e))?;

        // Configure the logger with this file as the output target
        let target = Box::new(file);

        builder.target(env_logger::Target::Pipe(target));
    }

    builder
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {}:{} {:?}] {}",
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                OffsetDateTime::now_utc(),
                record.args()
            )
        })
        .filter_level(log_level);

    Ok(builder)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn builder_when_verbosity_within_range_then_return_ok() {
        for verbosity in 0..=MAX_VERBOSITY {
            assert!(builder(verbosity, None).is_ok());
        }
    }

    #[test]
    fn builder_when_verbosity_above_maximum_then_return_verbosity_out_of_range() {
        let err = builder(MAX_VERBOSITY + 1, None).unwrap_err();

        assert!(matches!(err, LogError::VerbosityOutOfRange(5)));
        assert_eq!(err.to_string(), "Verbosity level must be between 0 and 4");
    }

    #[test]
    fn builder_when_log_file_writable_then_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let log_file = dir.path().join("ironplc.log");

        assert!(builder(0, Some(&log_file)).is_ok());
        assert!(log_file.exists());
    }

    #[test]
    fn builder_when_log_file_not_creatable_then_return_log_file_create() {
        let dir = tempfile::tempdir().unwrap();
        // The parent directory does not exist, so the file cannot be created.
        let log_file = dir.path().join("missing").join("ironplc.log");

        let err = builder(0, Some(&log_file)).unwrap_err();

        assert!(matches!(err, LogError::LogFileCreate(ref path, _) if path == &log_file));
        assert!(err.to_string().starts_with(&format!(
            "Unable to create log file {}.",
            log_file.display()
        )));
    }

    #[test]
    fn configure_when_verbosity_above_maximum_then_return_err() {
        // `configure` installs the process-wide logger on success, which can
        // only happen once per program, so only the failing path is exercised
        // here. The success path is covered through `builder`.
        assert!(configure(MAX_VERBOSITY + 1, None).is_err());
    }
}
