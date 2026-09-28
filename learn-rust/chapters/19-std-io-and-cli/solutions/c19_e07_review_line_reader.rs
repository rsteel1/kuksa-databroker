// c19_e07_review_line_reader.rs
//
// CONCEPT: code review — this file COMPILES, but has 3 planted bugs typical
//          of chapter 19 (a raw `Read::read` treated as if it always fills
//          the buffer in one call, a line buffer that's never cleared
//          between lines, and a final line without a trailing newline
//          silently dropped).
// GUIDE:   chapters/19-std-io-and-cli/guide.md  (section "Processing input line by line")
// DATABROKER: databroker/src/vss.rs — `parse_vss_from_reader` reads a
//             `BufRead` line by line; a reader that under-delivers on one
//             `read()` call, or a buffer that isn't reset between lines,
//             would corrupt the signal tree it builds.
// EXPECT:  tests fail: a "header" read from a slow source comes back
//          truncated, lines pick up leftover text from previous lines, and
//          a file that doesn't end with a newline loses its last line.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any function's signature.

use std::io::{BufRead, BufReader, Read};

/// Reads exactly `len` bytes of a fixed-size header from `reader`.
fn read_exact_header(mut reader: impl Read, len: usize) -> String {
    let mut buf = vec![0u8; len];
    // BUG 1: `read` is only guaranteed to fill *some* of the buffer (even
    // 0 bytes doesn't necessarily mean EOF for every source); a single
    // call isn't enough for a reader that trickles bytes in small chunks.
    // Loop until the buffer is full or the source is exhausted.
    let mut filled = 0;
    while filled < len {
        let n = reader.read(&mut buf[filled..]).expect("read failed");
        if n == 0 {
            break;
        }
        filled += n;
    }
    String::from_utf8_lossy(&buf[..filled]).into_owned()
}

/// Reads every line of `reader`, without the trailing newline.
fn read_all_lines(reader: impl Read) -> Vec<String> {
    let mut buffered = BufReader::new(reader);
    let mut lines = Vec::new();
    let mut line = String::new();
    loop {
        let bytes_read = buffered.read_line(&mut line).expect("read_line failed");
        if bytes_read == 0 {
            break;
        }
        // BUG 3: bailing out on a line with no trailing newline dropped
        // the last line of any input that doesn't end in `\n`. `read_line`
        // already hands back whatever it has at EOF, newline or not, so
        // just trim it like every other line instead of rejecting it.
        lines.push(line.trim_end_matches('\n').to_string());
        // BUG 2: `read_line` *appends* to its buffer; without clearing it
        // first, every line after the first still has every previous line
        // in front of it.
        line.clear();
    }
    lines
}

/// A reader that only ever hands back up to `chunk` bytes per call, no
/// matter how big the caller's buffer is — modelling a slow socket.
struct ChunkedReader<'a> {
    data: &'a [u8],
    chunk: usize,
}

impl Read for ChunkedReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.chunk.min(buf.len()).min(self.data.len());
        buf[..n].copy_from_slice(&self.data[..n]);
        self.data = &self.data[n..];
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_read_in_full_even_from_a_slow_source() {
        let reader = ChunkedReader {
            data: b"HEADER1234",
            chunk: 3,
        };
        assert_eq!(read_exact_header(reader, 10), "HEADER1234");
    }

    #[test]
    fn each_line_starts_from_an_empty_buffer() {
        let input: &[u8] = b"a\nb\nc\n";
        assert_eq!(read_all_lines(input), vec!["a", "b", "c"]);
    }

    #[test]
    fn last_line_without_a_newline_is_still_returned() {
        let input: &[u8] = b"a\nb\nc";
        assert_eq!(read_all_lines(input), vec!["a", "b", "c"]);
    }
}
