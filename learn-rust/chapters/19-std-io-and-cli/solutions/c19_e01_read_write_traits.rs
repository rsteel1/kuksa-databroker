// c19_e01_read_write_traits.rs
//
// CONCEPT: `std::io::Read`, `BufRead` and `Write`: code that is generic over
//          *where* bytes come from or go to (files, sockets, memory, stdin).
// GUIDE:   chapters/19-std-io-and-cli/guide.md  (section "Read, BufRead and Write")
// DATABROKER: databroker/src/vss.rs — `parse_vss_from_reader<R: std::io::Read>`
//             (line 533); databroker/src/main.rs wraps the opened file in a
//             `BufReader` before handing it over (lines 159-161).
// EXPECT:  tests fail: `todo!()` panics. When you implement `first_line` you
//          will hit E0599 "no method named `read_line` found for type
//          parameter `R`": the bound is `Read`, but `read_line` is on `BufRead`.
//
// Methods come from traits, and a generic function can only call what its
// bounds promise. `Read::read` fills a buffer with *some* bytes (maybe fewer
// than asked, maybe 0 at end of input). `BufRead` adds an internal buffer and
// line-oriented helpers. `Write::write_all` + `flush` is the output side.
// Tests use `&[u8]`, `Cursor` and `Vec<u8>`: in-memory types that implement
// the same traits as `File` and `TcpStream`.
//
// YOUR TASK
//   Implement the `todo!()`s (and fix `first_line`'s bound when needed):
//     - `copy_uppercase`: read ALL input in chunks with `Read::read`, write
//       it ASCII-uppercased, return the number of bytes copied; retry on
//       `ErrorKind::Interrupted`, propagate any other error;
//     - `first_line`: the first line without its `\n` / `\r\n`, or `None` at
//       end of input;
//     - `write_report`: one line per entry, the path left-aligned in 20
//       columns, a space, the value right-aligned in 8 columns with 2
//       decimals, then flush.
//   Do not edit the tests.

use std::io::{self, BufRead, ErrorKind, Read, Write};

fn copy_uppercase<R: Read, W: Write>(mut reader: R, mut writer: W) -> io::Result<u64> {
    // WHY: a small fixed buffer keeps memory use constant however big the
    // input is; real code would use 8 KiB (what `io::copy` uses).
    let mut buf = [0u8; 16];
    let mut total = 0u64;
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break, // end of input
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        let chunk = &mut buf[..n];
        chunk.make_ascii_uppercase();
        // WHY: `write` may accept only part of the chunk; `write_all` loops.
        writer.write_all(chunk)?;
        total += n as u64;
    }
    Ok(total)
}

fn first_line<R: BufRead>(mut reader: R) -> io::Result<Option<String>> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let len = line.trim_end_matches(['\n', '\r']).len();
    line.truncate(len);
    Ok(Some(line))
}

fn write_report<W: Write>(mut out: W, entries: &[(&str, f64)]) -> io::Result<()> {
    for (path, value) in entries {
        writeln!(out, "{path:<20} {value:>8.2}")?;
    }
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A reader that fails with `Interrupted` once, then serves `data`.
    struct Flaky {
        interrupted: bool,
        data: Cursor<Vec<u8>>,
    }

    impl Read for Flaky {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::Error::new(ErrorKind::Interrupted, "signal"));
            }
            self.data.read(buf)
        }
    }

    /// A writer whose disk is always full.
    struct DiskFull;

    impl Write for DiskFull {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(ErrorKind::StorageFull, "disk full"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn copies_everything_uppercased() {
        let input = b"vehicle.speed = 42\nvehicle.cabin.temperature = 21.5\n";
        let mut out = Vec::new();
        let n = copy_uppercase(&input[..], &mut out).unwrap();
        assert_eq!(n, input.len() as u64);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "VEHICLE.SPEED = 42\nVEHICLE.CABIN.TEMPERATURE = 21.5\n"
        );
    }

    #[test]
    fn retries_when_interrupted() {
        let reader = Flaky {
            interrupted: false,
            data: Cursor::new(b"abc".to_vec()),
        };
        let mut out = Vec::new();
        assert_eq!(copy_uppercase(reader, &mut out).unwrap(), 3);
        assert_eq!(out, b"ABC");
    }

    #[test]
    fn write_errors_propagate() {
        let err = copy_uppercase(&b"data"[..], DiskFull).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::StorageFull);
    }

    #[test]
    fn first_line_strips_line_endings() {
        assert_eq!(
            first_line(Cursor::new("Vehicle.Speed\r\nnext")).unwrap(),
            Some("Vehicle.Speed".to_string())
        );
        assert_eq!(
            first_line(&b"no newline"[..]).unwrap(),
            Some("no newline".to_string())
        );
        assert_eq!(first_line(&b""[..]).unwrap(), None);
    }

    #[test]
    fn a_mutable_reference_is_also_a_reader() {
        // `impl<B: BufRead + ?Sized> BufRead for &mut B` lets us lend the
        // reader out and keep reading where it stopped.
        let mut cursor = Cursor::new("one\ntwo\n");
        assert_eq!(first_line(&mut cursor).unwrap().as_deref(), Some("one"));
        assert_eq!(first_line(&mut cursor).unwrap().as_deref(), Some("two"));
        assert_eq!(first_line(&mut cursor).unwrap(), None);
    }

    #[test]
    fn report_is_aligned() {
        let mut out = Vec::new();
        write_report(
            &mut out,
            &[("Vehicle.Speed", 42.0), ("Vehicle.Cabin.Temp", -3.456)],
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Vehicle.Speed           42.00\nVehicle.Cabin.Temp      -3.46\n"
        );
    }
}
