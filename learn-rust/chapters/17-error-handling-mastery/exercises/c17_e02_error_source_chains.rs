// c17_e02_error_source_chains.rs
//
// CONCEPT: errors form a chain through `Error::source()`. Each layer adds
//          *what it was doing*, and the layer below keeps *why it failed*.
//          thiserror's `#[source]` and `#[from]` wire the chain up for you.
// GUIDE:   chapters/17-error-handling-mastery/guide.md  (section "Error chains: source() and context")
// DATABROKER: databroker/src/vss.rs:167 — `impl From<serde_json::Error> for Error`
//             does `Error::ParseError(from.to_string())`: it flattens the
//             serde error into a String, so `source()` is gone and callers can
//             no longer ask *which line/column* failed.
// EXPECT:  tests fail: `todo!()` panics, and `source()` returns None until
//          the missing `#[source]` / `#[from]` attributes are added
//
// A good error report reads top-down, from the most general to the root cause:
//
//     failed to load VSS overlay "overlay.json"
//     caused by: bad value on line 3
//     caused by: invalid digit found in string
//
// Each line is one error in the chain. The top error doesn't repeat its
// cause in its own message; `report` walks `source()` to print the rest.
//
// YOUR TASK
//   1. Add the attributes (see `// FIX` comments) so every error exposes its
//      cause through `source()`.
//   2. Implement `report` and `root_cause`.
//   Do not edit the tests.

// I AM NOT DONE

use std::error::Error;
use std::io;
use std::num::ParseIntError;

#[derive(Debug, thiserror::Error)]
enum ParseError {
    #[error("bad value on line {line}")]
    BadValue {
        line: usize,
        // FIX: this field is the cause of the error: mark it as the source
        cause: ParseIntError,
    },
}

#[derive(Debug, thiserror::Error)]
enum LoadError {
    #[error("failed to read {path:?}")]
    Read {
        path: String,
        // FIX: mark as the source
        cause: io::Error,
    },
    #[error("failed to load VSS overlay {path:?}")]
    Parse {
        path: String,
        // FIX: mark as the source
        cause: ParseError,
    },
}

/// Parses one number per line, reporting the (1-based) line that failed.
fn parse_lines(text: &str) -> Result<Vec<i64>, ParseError> {
    text.lines()
        .enumerate()
        .map(|(i, l)| {
            l.trim()
                .parse::<i64>()
                .map_err(|cause| ParseError::BadValue { line: i + 1, cause })
        })
        .collect()
}

/// Simulated file load: `None` content means "file missing".
fn load(path: &str, content: Option<&str>) -> Result<Vec<i64>, LoadError> {
    let text = content.ok_or_else(|| LoadError::Read {
        path: path.to_string(),
        cause: io::Error::new(io::ErrorKind::NotFound, "no such file"),
    })?;
    parse_lines(text).map_err(|cause| LoadError::Parse {
        path: path.to_string(),
        cause,
    })
}

/// Renders the whole chain: the top error, then one "caused by: ..." line
/// per source, joined with '\n'.
fn report(err: &dyn Error) -> String {
    todo!()
}

/// The last error in the chain (the error itself if it has no source).
fn root_cause<'e>(err: &'e (dyn Error + 'static)) -> &'e (dyn Error + 'static) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_is_wired_up() {
        let err = load("overlay.json", Some("1\n2\nx3")).unwrap_err();
        let parse = err
            .source()
            .expect("LoadError::Parse must expose its source");
        assert!(
            parse.source().is_some(),
            "ParseError::BadValue must expose its source"
        );
        let missing = load("gone.json", None).unwrap_err();
        assert!(
            missing.source().is_some(),
            "LoadError::Read must expose its source"
        );
    }

    #[test]
    fn full_report() {
        let err = load("overlay.json", Some("1\n2\nx3")).unwrap_err();
        assert_eq!(
            report(&err),
            "failed to load VSS overlay \"overlay.json\"\n\
             caused by: bad value on line 3\n\
             caused by: invalid digit found in string"
        );
    }

    #[test]
    fn report_without_cause() {
        let err = io::Error::other("plain");
        assert_eq!(report(&err), "plain");
    }

    #[test]
    fn root_cause_is_the_parse_int_error() {
        let err = load("overlay.json", Some("oops")).unwrap_err();
        let root = root_cause(&err);
        assert!(root.downcast_ref::<ParseIntError>().is_some());

        let missing = load("gone.json", None).unwrap_err();
        let root = root_cause(&missing);
        let io = root
            .downcast_ref::<io::Error>()
            .expect("root is an io::Error");
        assert_eq!(io.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn happy_path() {
        assert_eq!(load("ok.json", Some("1\n-2\n 3 ")).unwrap(), vec![1, -2, 3]);
    }
}
