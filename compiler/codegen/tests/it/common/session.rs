//! A program driven across several scans, read and written by name.

use super::backend::Running;
use super::value::{convert, FromValue, Value};

/// A loaded program driven across several scans.
///
/// Variables are read and written by name, as with
/// [`Snapshot`](super::Snapshot); values persist from one scan to the next.
pub struct Session<'r, F> {
    running: &'r mut dyn Running<Fault = F>,
}

impl<'r, F> Session<'r, F> {
    pub(super) fn new(running: &'r mut dyn Running<Fault = F>) -> Self {
        Session { running }
    }

    /// Runs one scan at absolute time `time_us` (microseconds).
    pub fn scan(&mut self, time_us: u64) -> Result<(), F> {
        self.running.scan(time_us)
    }

    /// The value of the program or global variable `name`.
    pub fn read(&self, name: &str) -> Value {
        self.running.read(name)
    }

    /// The value of `name` as a `T`. Fails the test when the value cannot be
    /// converted to a `T` without loss.
    pub fn read_as<T: FromValue>(&self, name: &str) -> T {
        convert(name, &self.running.declared(name), &self.read(name))
    }

    /// Writes `value` to the variable `name`; the next scan reads it. Fails
    /// the test when the variable's declared type cannot hold `value`.
    pub fn write(&mut self, name: &str, value: impl Into<Value>) {
        self.running.write(name, value.into());
    }
}
