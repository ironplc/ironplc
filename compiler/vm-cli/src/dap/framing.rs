//! Content-Length framing for the Debug Adapter Protocol.
//!
//! DAP messages are exchanged over a byte stream (stdin/stdout) using the
//! same framing as the Language Server Protocol: a `Content-Length` header,
//! terminated by a blank line, followed by exactly that many bytes of JSON
//! body:
//!
//! ```text
//! Content-Length: 42\r\n
//! \r\n
//! {"seq":1,"type":"request", ... }
//! ```
//!
//! This module handles only the framing — turning a stream of bytes into
//! discrete message bodies and back. It knows nothing about JSON or the DAP
//! message shapes; those live in `dap::types`.

use std::io::{self, BufRead, Write};

/// Writes a single DAP message: the `Content-Length` header, the blank-line
/// separator, then `body`. Flushes so the peer sees the message immediately.
pub fn write_message<W: Write>(writer: &mut W, body: &[u8]) -> io::Result<()> {
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(body)?;
    writer.flush()
}

/// Reads a single DAP message body from `reader`.
///
/// Returns `Ok(Some(body))` for a complete message, or `Ok(None)` on a clean
/// end-of-stream *before* any header bytes (the peer disconnected between
/// messages). Fragmented input is handled transparently: the underlying
/// [`BufRead`] loops until each header line and the full body have arrived.
///
/// Errors with [`io::ErrorKind::InvalidData`] if the header block is missing
/// a valid `Content-Length`, or if the stream ends partway through a message.
pub fn read_message<R: BufRead>(reader: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut content_length: Option<usize> = None;
    let mut saw_any_header_byte = false;

    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            // End of stream. Clean only if it lands exactly on a message
            // boundary (no partial header block was in progress).
            if saw_any_header_byte {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "stream ended in the middle of a message header",
                ));
            }
            return Ok(None);
        }
        saw_any_header_byte = true;

        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            // Blank line: end of the header block.
            break;
        }
        if let Some(value) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(value.trim().parse().map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "invalid Content-Length value")
            })?);
        }
        // Any other header (e.g. `Content-Type`) is accepted and ignored.
    }

    let len = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
    })?;

    let mut body = vec![0u8; len];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    #[test]
    fn write_message_when_body_given_then_frames_with_content_length() {
        let mut out = Vec::new();
        write_message(&mut out, b"{}").unwrap();
        assert_eq!(out, b"Content-Length: 2\r\n\r\n{}");
    }

    #[test]
    fn read_message_when_well_framed_then_returns_body() {
        let mut input = Cursor::new(b"Content-Length: 2\r\n\r\n{}".to_vec());
        let body = read_message(&mut input).unwrap();
        assert_eq!(body.as_deref(), Some(&b"{}"[..]));
    }

    #[test]
    fn roundtrip_when_written_then_read_back_identically() {
        let payload = br#"{"seq":1,"type":"request","command":"initialize"}"#;
        let mut buf = Vec::new();
        write_message(&mut buf, payload).unwrap();

        let mut reader = Cursor::new(buf);
        let body = read_message(&mut reader).unwrap();
        assert_eq!(body.as_deref(), Some(&payload[..]));
    }

    #[test]
    fn read_message_when_multiple_messages_buffered_then_reads_each_in_order() {
        let mut buf = Vec::new();
        write_message(&mut buf, b"first").unwrap();
        write_message(&mut buf, b"second").unwrap();

        let mut reader = Cursor::new(buf);
        assert_eq!(
            read_message(&mut reader).unwrap().as_deref(),
            Some(&b"first"[..])
        );
        assert_eq!(
            read_message(&mut reader).unwrap().as_deref(),
            Some(&b"second"[..])
        );
        // Nothing left: clean end-of-stream.
        assert_eq!(read_message(&mut reader).unwrap(), None);
    }

    /// A reader that hands out at most one byte per `read` call, to prove the
    /// framing tolerates a fragmented transport.
    struct OneByteAtATime<R: Read> {
        inner: R,
    }
    impl<R: Read> Read for OneByteAtATime<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }
            self.inner.read(&mut buf[..1])
        }
    }

    #[test]
    fn read_message_when_input_fragmented_then_still_reassembles() {
        let mut buf = Vec::new();
        write_message(&mut buf, b"chunky").unwrap();

        let mut reader = io::BufReader::new(OneByteAtATime {
            inner: Cursor::new(buf),
        });
        let body = read_message(&mut reader).unwrap();
        assert_eq!(body.as_deref(), Some(&b"chunky"[..]));
    }

    #[test]
    fn read_message_when_empty_stream_then_returns_none() {
        let mut reader = Cursor::new(Vec::new());
        assert_eq!(read_message(&mut reader).unwrap(), None);
    }

    #[test]
    fn read_message_when_missing_content_length_then_errors() {
        let mut reader = Cursor::new(b"Content-Type: application/json\r\n\r\n".to_vec());
        let err = read_message(&mut reader).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn read_message_when_other_headers_present_then_ignores_them() {
        let mut reader =
            Cursor::new(b"Content-Type: application/json\r\nContent-Length: 2\r\n\r\n{}".to_vec());
        let body = read_message(&mut reader).unwrap();
        assert_eq!(body.as_deref(), Some(&b"{}"[..]));
    }
}
