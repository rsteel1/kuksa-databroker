// c19_e03_line_processing.rs
//
// CONCEPT: streaming line-by-line processing with `BufRead::lines()` /
//          `read_line`: constant memory however large the input, line
//          numbers for error messages, and invalid UTF-8 handling.
// GUIDE:   chapters/19-std-io-and-cli/guide.md  (section "Processing input line by line")
// DATABROKER: databroker/src/main.rs — the file is wrapped in a `BufReader`
//             before parsing (line 160); databroker-cli/src/kuksa_cli.rs reads
//             the user's commands line by line in its interactive loop.
// EXPECT:  tests fail: `todo!()` panics.
//
// Reading a whole file into a `String` is fine for small config files, but a
// recorded signal log can be gigabytes. A `BufReader` pulls bytes in chunks
// (8 KiB by default), and `lines()` hands you one line at a time, so memory
// use stays flat. The tests feed a generated input of 100 000 lines that is
// never materialised as one string.
//
// Input format, one update per line:   Vehicle.Speed=42.5
//   - blank lines and lines starting with `#` are skipped;
//   - anything else that doesn't parse is an error (record its 1-based line
//     number and keep going);
//   - an I/O error (including invalid UTF-8, `ErrorKind::InvalidData`) stops
//     processing and is returned.
//
// YOUR TASK
//   Implement `summarize`. Do not edit `Summary` or the tests.

use std::collections::BTreeMap;
use std::io::{self, BufRead};

#[derive(Debug, Default, PartialEq)]
struct Summary {
    /// Number of valid updates.
    updates: usize,
    /// Blank and comment lines.
    skipped: usize,
    /// 1-based line numbers of lines that could not be parsed.
    bad_lines: Vec<usize>,
    /// Largest value seen per path.
    max_by_path: BTreeMap<String, f64>,
}

fn parse_update(line: &str) -> Option<(&str, f64)> {
    let (path, value) = line.split_once('=')?;
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    Some((path, value.trim().parse().ok()?))
}

fn summarize<R: BufRead>(reader: R) -> io::Result<Summary> {
    let mut summary = Summary::default();
    // WHY: `enumerate` gives the index; line numbers for humans start at 1.
    for (index, line) in reader.lines().enumerate() {
        // WHY: each item is `io::Result<String>`; `?` stops on I/O errors
        // (including invalid UTF-8) while *parse* errors are just recorded.
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            summary.skipped += 1;
            continue;
        }
        match parse_update(trimmed) {
            Some((path, value)) => {
                summary.updates += 1;
                summary
                    .max_by_path
                    .entry(path.to_string())
                    .and_modify(|max| *max = max.max(value))
                    .or_insert(value);
            }
            None => summary.bad_lines.push(index + 1),
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor, ErrorKind, Read};

    /// Produces `total` lines like "Vehicle.Speed=17\n" on demand, without
    /// ever holding more than one line in memory.
    struct Generated {
        next: usize,
        total: usize,
        pending: Vec<u8>,
    }

    impl Read for Generated {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.pending.is_empty() {
                if self.next == self.total {
                    return Ok(0);
                }
                let path = if self.next.is_multiple_of(2) {
                    "Vehicle.Speed"
                } else {
                    "Vehicle.Rpm"
                };
                self.pending = format!("{path}={}\n", self.next % 1000).into_bytes();
                self.next += 1;
            }
            let n = buf.len().min(self.pending.len());
            buf[..n].copy_from_slice(&self.pending[..n]);
            self.pending.drain(..n);
            Ok(n)
        }
    }

    #[test]
    fn small_input() {
        let input = "\
# recorded 2026-09-25
Vehicle.Speed=42.5

Vehicle.Speed=10
Vehicle.Cabin.Temperature = 21.5
garbage
Vehicle.Rpm=abc
=3
";
        let summary = summarize(Cursor::new(input)).unwrap();
        assert_eq!(summary.updates, 3);
        assert_eq!(summary.skipped, 2);
        assert_eq!(summary.bad_lines, vec![6, 7, 8]);
        assert_eq!(summary.max_by_path["Vehicle.Speed"], 42.5);
        assert_eq!(summary.max_by_path["Vehicle.Cabin.Temperature"], 21.5);
    }

    #[test]
    fn empty_input() {
        assert_eq!(summarize(Cursor::new("")).unwrap(), Summary::default());
    }

    #[test]
    fn streams_a_large_input() {
        let reader = BufReader::new(Generated {
            next: 0,
            total: 100_000,
            pending: Vec::new(),
        });
        let summary = summarize(reader).unwrap();
        assert_eq!(summary.updates, 100_000);
        assert!(summary.bad_lines.is_empty());
        assert_eq!(summary.max_by_path.len(), 2);
        assert_eq!(summary.max_by_path["Vehicle.Speed"], 998.0);
        assert_eq!(summary.max_by_path["Vehicle.Rpm"], 999.0);
    }

    #[test]
    fn invalid_utf8_is_an_io_error() {
        let bytes: &[u8] = b"Vehicle.Speed=1\n\xff\xfe\nVehicle.Speed=2\n";
        let err = summarize(bytes).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidData);
    }
}
