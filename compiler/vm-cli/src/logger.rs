//! Provides configuration of a logger.
//!
//! The logger itself is shared with the other IronPLC command line programs
//! (see `ironplc_cli_support::logger`). This module owns only how a
//! configuration failure is reported to the user of `ironplcvm`.
use std::path::PathBuf;

use crate::error::{self, VmError};

/// Configures the log with the specified verbosity.
///
/// Higher verbosity results in additional log messages
/// up to a maximum verbosity level.
pub fn configure(verbosity: u8, log_file: Option<PathBuf>) -> Result<(), VmError> {
    ironplc_cli_support::logger::configure(verbosity, log_file)
        .map_err(|err| VmError::io(error::LOG_CONFIG, err.to_string()))
}

#[cfg(test)]
mod test {
    use crate::logger::configure;

    #[test]
    fn configure_when_verbosity_is_5_then_return_v6007() {
        let result = configure(5, None);

        let err = result.unwrap_err();
        assert!(err.to_string().starts_with("V6007"));
    }
}
