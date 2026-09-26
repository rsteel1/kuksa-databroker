// c19_e05_cli_program_end_to_end.rs
//
// CONCEPT: a complete command-line program whose logic is fully testable:
//          `main` is a thin shell around `run(args, stdin, stdout, stderr)`.
// GUIDE:   chapters/19-std-io-and-cli/guide.md  (section "A testable CLI program")
// DATABROKER: databroker/src/main.rs — `read_metadata_file` (line 153) is the
//             same "open file → BufReader → parse" pipeline; databroker-cli
//             is a full CLI built on the same ideas (plus clap, chapter 31).
// EXPECT:  tests fail: `todo!()` panics.
//
// `vss-grep` prints the VSS paths (one per input line) that contain a pattern.
//
//   usage: vss-grep [-i] [-c] <pattern> [file]
//     -i   case-insensitive match
//     -c   print only the number of matching lines
//     file read this file instead of standard input
//
// Exit codes follow grep: 0 = at least one match, 1 = no match,
// 2 = usage error, 74 = I/O error (as in c19_e04).
//
// A real `main` would be:
//
//   fn main() -> std::process::ExitCode {
//       let args: Vec<String> = std::env::args().skip(1).collect();
//       let code = run(&args, std::io::stdin().lock(), std::io::stdout().lock(),
//                      std::io::stderr().lock());
//       std::process::ExitCode::from(code)
//   }
//
// YOUR TASK
//   Implement `parse_args` and `run`. Error messages go to `stderr`, start
//   with "vss-grep: " and end with a newline; usage errors also print the
//   USAGE line. Do not edit `Options`, `USAGE` or the tests.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: vss-grep [-i] [-c] <pattern> [file]";

#[derive(Debug, PartialEq)]
struct Options {
    ignore_case: bool,
    count_only: bool,
    pattern: String,
    file: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut ignore_case = false;
    let mut count_only = false;
    let mut positional = Vec::new();
    for arg in args {
        match arg.as_str() {
            "-i" => ignore_case = true,
            "-c" => count_only = true,
            flag if flag.starts_with('-') && flag.len() > 1 => {
                return Err(format!("unknown option '{flag}'"))
            }
            _ => positional.push(arg.clone()),
        }
    }
    // WHY: a slice pattern states the three valid shapes directly.
    match positional.as_slice() {
        [] => Err("missing <pattern>".to_string()),
        [pattern] => Ok(Options {
            ignore_case,
            count_only,
            pattern: pattern.clone(),
            file: None,
        }),
        [pattern, file] => Ok(Options {
            ignore_case,
            count_only,
            pattern: pattern.clone(),
            file: Some(file.clone()),
        }),
        [_, _, extra, ..] => Err(format!("unexpected argument '{extra}'")),
    }
}

/// Writes matching lines (or the count) and returns how many matched.
fn grep<R: BufRead, W: Write>(input: R, mut out: W, opts: &Options) -> io::Result<usize> {
    let needle = if opts.ignore_case {
        opts.pattern.to_lowercase()
    } else {
        opts.pattern.clone()
    };
    let mut matches = 0;
    for line in input.lines() {
        let line = line?;
        let found = if opts.ignore_case {
            line.to_lowercase().contains(&needle)
        } else {
            line.contains(&needle)
        };
        if found {
            matches += 1;
            if !opts.count_only {
                writeln!(out, "{line}")?;
            }
        }
    }
    if opts.count_only {
        writeln!(out, "{matches}")?;
    }
    out.flush()?;
    Ok(matches)
}

fn run<R: BufRead, W: Write, E: Write>(args: &[String], stdin: R, stdout: W, mut stderr: E) -> u8 {
    let opts = match parse_args(args) {
        Ok(opts) => opts,
        Err(message) => {
            // WHY: if even stderr fails there's nobody left to tell; ignore.
            let _ = writeln!(stderr, "vss-grep: {message}\n{USAGE}");
            return 2;
        }
    };
    let result = match &opts.file {
        Some(path) => match File::open(path) {
            Ok(file) => grep(BufReader::new(file), stdout, &opts),
            Err(e) => {
                let _ = writeln!(stderr, "vss-grep: {path}: {e}");
                return 74;
            }
        },
        None => grep(stdin, stdout, &opts),
    };
    match result {
        Ok(0) => 1,
        Ok(_) => 0,
        Err(e) => {
            let _ = writeln!(stderr, "vss-grep: {e}");
            74
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const SIGNALS: &str = "\
Vehicle.Speed
Vehicle.Cabin.Door.Row1.DriverSide.IsOpen
Vehicle.Cabin.Door.Row1.PassengerSide.IsOpen
Vehicle.Powertrain.TractionBattery.StateOfCharge.Current
";

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Runs the program with `SIGNALS` on stdin; returns (code, stdout, stderr).
    fn call(list: &[&str]) -> (u8, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run(&args(list), Cursor::new(SIGNALS), &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn parses_flags_in_any_order() {
        assert_eq!(
            parse_args(&args(&["-c", "Door", "-i", "signals.txt"])),
            Ok(Options {
                ignore_case: true,
                count_only: true,
                pattern: "Door".into(),
                file: Some("signals.txt".into()),
            })
        );
        assert!(parse_args(&args(&[])).is_err());
        assert!(parse_args(&args(&["-x", "a"])).unwrap_err().contains("-x"));
        assert!(parse_args(&args(&["a", "b", "c"]))
            .unwrap_err()
            .contains("'c'"));
    }

    #[test]
    fn prints_matching_lines() {
        let (code, out, err) = call(&["IsOpen"]);
        assert_eq!(code, 0);
        assert_eq!(
            out,
            "Vehicle.Cabin.Door.Row1.DriverSide.IsOpen\nVehicle.Cabin.Door.Row1.PassengerSide.IsOpen\n"
        );
        assert_eq!(err, "");
    }

    #[test]
    fn case_insensitive_count() {
        let (code, out, _) = call(&["-i", "-c", "isopen"]);
        assert_eq!(code, 0);
        assert_eq!(out, "2\n");
    }

    #[test]
    fn no_match_exits_1() {
        let (code, out, _) = call(&["Wiper"]);
        assert_eq!(code, 1);
        assert_eq!(out, "");
        let (code, out, _) = call(&["-c", "Wiper"]);
        assert_eq!((code, out.as_str()), (1, "0\n"));
    }

    #[test]
    fn usage_error_exits_2() {
        let (code, out, err) = call(&[]);
        assert_eq!(code, 2);
        assert_eq!(out, "");
        assert_eq!(err, format!("vss-grep: missing <pattern>\n{USAGE}\n"));
    }

    #[test]
    fn reads_a_file() {
        let path =
            std::env::temp_dir().join(format!("learn-rust-c19e05-{}.txt", std::process::id()));
        std::fs::write(&path, "Vehicle.Speed\nVehicle.Rpm\n").unwrap();
        let mut out = Vec::new();
        let code = run(
            &args(&["Rpm", path.to_str().unwrap()]),
            Cursor::new("ignored stdin: Rpm"),
            &mut out,
            io::sink(),
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(code, 0);
        assert_eq!(String::from_utf8(out).unwrap(), "Vehicle.Rpm\n");
    }

    #[test]
    fn missing_file_exits_74() {
        let mut err = Vec::new();
        let code = run(
            &args(&["x", "/no/such/signals.txt"]),
            Cursor::new(""),
            io::sink(),
            &mut err,
        );
        assert_eq!(code, 74);
        let err = String::from_utf8(err).unwrap();
        assert!(err.starts_with("vss-grep: /no/such/signals.txt: "), "{err}");
        assert!(err.ends_with('\n'));
    }
}
