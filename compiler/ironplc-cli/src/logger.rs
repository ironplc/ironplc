//! Provides configuration of a logger.
//!
//! The logger itself is shared with the other IronPLC command line programs
//! (see `ironplc_cli_support::logger`). This module owns only how a
//! configuration failure is reported to the user of `ironplcc`.
use ironplc_cli_support::logger::LogError;
use std::path::PathBuf;

/// Configures the log with the specified verbosity.
///
/// Higher verbosity results in additional log messages
/// up to a maximum verbosity level.
pub fn configure(verbosity: u8, log_file: Option<PathBuf>) -> Result<(), String> {
    ironplc_cli_support::logger::configure(verbosity, log_file).map_err(|err| match err {
        LogError::VerbosityOutOfRange(_) => String::from("Don't be crazy with verbose"),
        LogError::LogFileCreate(..) => err.to_string(),
    })
}

#[cfg(test)]
mod test {
    use crate::logger::configure;

    #[test]
    fn configure_when_verbosity_is_5_then_return_err() {
        let result = configure(5, None);

        assert_eq!(result.unwrap_err(), "Don't be crazy with verbose");
    }
}
