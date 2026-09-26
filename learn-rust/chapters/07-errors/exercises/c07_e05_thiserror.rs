// c07_e05_thiserror.rs
//
// CONCEPT: `thiserror` — derive Display, Error and From instead of writing them.
// GUIDE:   chapters/07-errors/guide.md  (section "thiserror: the same, derived")
// DATABROKER: databroker/src/authorization/mod.rs — `#[derive(Error, Debug)]
//             pub enum Error { #[error("Invalid public key")] InvalidPublicKey }`.
//             Most other errors are hand-written enums (compare vss.rs `Error`).
// EXPECT:  compile errors: E0599 (no method `to_string` / `source` on
//          `ProviderError`) and E0277 (`From<std::io::Error>` is not implemented).
//
// Exercises e03 and e04 took ~30 lines of boilerplate. `thiserror` generates
// exactly that code at compile time from attributes:
//
//   #[derive(Debug, thiserror::Error)]
//   enum MyError {
//       #[error("signal not found: {0}")]      -> Display message
//       NotFound(String),
//       #[error("bad json")]
//       Json(#[from] serde_json::Error),     -> impl From + source()
//   }
//
// YOUR TASK
//   Add `#[derive(thiserror::Error)]` and the right `#[error(...)]` /
//   `#[from]` / `#[source]` attributes to `ProviderError` so that the code
//   compiles and the tests pass. Don't hand-write any impl blocks.
//   Do not edit `register` or the tests.

// I AM NOT DONE

use std::error::Error as _; // brings `.source()` into scope for the tests

#[derive(Debug)]
enum ProviderError {
    /// message: "signal already provided: {id}"
    AlreadyProvided { id: i32 },

    /// message: "invalid signal id `<the String field>`"; the ParseIntError
    /// must be reported as the error's `source()`. (Hint: in `#[error]`,
    /// tuple fields are written `{0}`, `{1}`, ...)
    InvalidId(std::num::ParseIntError, String),

    /// message: "provider stream closed"  (converts from io::Error, keeps source)
    Closed(std::io::Error),
}

fn register(raw_id: &str, taken: &[i32]) -> Result<i32, ProviderError> {
    let id: i32 = raw_id
        .parse()
        .map_err(|e| ProviderError::InvalidId(e, raw_id.to_string()))?;
    if taken.contains(&id) {
        return Err(ProviderError::AlreadyProvided { id });
    }
    Ok(id)
}

fn send(ok: bool) -> Result<(), ProviderError> {
    if ok {
        Ok(())
    } else {
        // `.into()` converts io::Error -> ProviderError via the generated From impl.
        Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "gone").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages() {
        assert_eq!(
            register("7", &[7]).unwrap_err().to_string(),
            "signal already provided: 7"
        );
        assert_eq!(
            register("x7", &[]).unwrap_err().to_string(),
            "invalid signal id `x7`"
        );
        assert_eq!(
            send(false).unwrap_err().to_string(),
            "provider stream closed"
        );
        assert_eq!(register("12", &[1]).unwrap(), 12);
    }

    #[test]
    fn sources_are_kept() {
        let err = register("x7", &[]).unwrap_err();
        let source = err
            .source()
            .expect("InvalidId must expose its ParseIntError");
        assert_eq!(source.to_string(), "invalid digit found in string");

        let err = send(false).unwrap_err();
        assert_eq!(err.source().expect("io source").to_string(), "gone");

        assert!(register("7", &[7]).unwrap_err().source().is_none());
    }
}
