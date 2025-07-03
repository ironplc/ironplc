//! Provides configuration of a logger.
use env_logger::Builder;
use log::trace;
use log::LevelFilter;
use std::fs::File;
use std::io::Write;
use std::{env, fs};
use time::OffsetDateTime;

/// Configures the log with the specified verbosity.
///
/// Higher verbosity results in additional log messages
/// up to a maximum verbosity level.
pub fn configure(verbosity: u8) -> Result<(), String> {
    let log_level = match verbosity {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        4 => LevelFilter::Trace,
        _ => return Err(String::from("Don't be crazy with verbose")),
    };

    trace!("Logger verbosity {log_level}");

    // Determine the output log file - first the path then then file
    // This path is important for the end-to-end smoke test.
    let log_location = env::temp_dir().join("ironplcc");
    fs::create_dir_all(&log_location).map_err(|e| {
        format!(
            "Unable to create log file {}. {}",
            log_location.display(),
            e
        )
    })?;

    let log_location = log_location.join("ironplcc.log");
    let file = File::create(&log_location).map_err(|e| {
        format!(
            "Unable to create log file {}. {}",
            log_location.display(),
            e
        )
    })?;

    // Configure the logger with this file as the output target
    let target = Box::new(file);
    Builder::new()
        .target(env_logger::Target::Pipe(target))
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
        .filter_level(log_level)
        .init();

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::logger::configure;

    #[test]
    fn configure_when_verbosity_is_5_then_return_err() {
        let result = configure(5);

        assert!(result.is_err());
    }
}
