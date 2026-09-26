// c34_e02_read_anyhow_error.rs
//
// CONCEPT: read-then-rebuild `anyhow`: one type-erased error that can wrap
//          any `std::error::Error`, context layers that form a `source()`
//          chain, and the exact `Display`/`Debug` formats you see when a
//          program prints `Error: ...` and `Caused by:`.
// GUIDE:   chapters/34-reading-any-codebase/guide.md  (section "Reading trait-heavy and generic-heavy code")
// DATABROKER: databroker/build.rs — the build script returns `anyhow::Result`;
//             databroker/src/main.rs returns `Box<dyn std::error::Error>`,
//             anyhow's simpler cousin.
// EXPECT:  tests fail: `todo!()` panics
//
// READ FIRST (anyhow 1.0.104, under ~/.cargo/registry/src/index.crates.io-*/anyhow-1.0.104/):
//   src/lib.rs:390      `pub struct Error` — a single thin pointer
//   src/error.rs:674    `impl<E> From<E> for Error where E: StdError + Send + Sync + 'static`
//                       (this is what makes `?` work on any error)
//   src/lib.rs:616      `pub trait Context<T, E>` — `.context()` / `.with_context()`
//   src/error.rs:926    `struct ContextError<C, E> { context, error }`
//   src/context.rs:137  its `StdError` impl: `source()` returns the wrapped error
//   src/fmt.rs:20       `debug()` — the "Caused by:" format, numbered when there
//                       are several causes
// The real crate uses a hand-built vtable and `unsafe` so `Error` is one
// pointer wide. You'll use `Box<dyn Error + Send + Sync>` instead: same
// behaviour, simpler machinery.
//
// YOUR TASK
//   Implement the `todo!()`s. The last test prints the same error chain with
//   real anyhow and with yours; the text must match. Do not edit the tests.

use std::error::Error as StdError;
use std::fmt::{self, Debug, Display};

type DynError = Box<dyn StdError + Send + Sync + 'static>;

/// A type-erased error with a chain of causes.
///
/// Like anyhow's, it deliberately does NOT implement `std::error::Error`:
/// if it did, `impl<E: StdError> From<E> for Error` would overlap with the
/// standard library's `impl<T> From<T> for T`.
pub struct Error {
    inner: DynError,
}

impl<E> From<E> for Error
where
    E: StdError + Send + Sync + 'static,
{
    fn from(error: E) -> Self {
        Error {
            inner: Box::new(error),
        }
    }
}

/// An error that is only a message (what `anyhow!("...")` creates).
struct MessageError<M>(M);

impl<M: Display> Display for MessageError<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<M: Debug> Debug for MessageError<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<M: Display + Debug> StdError for MessageError<M> {}

/// One context layer wrapping the previous error.
struct ContextError<C> {
    context: C,
    error: DynError,
}

impl<C: Display> Display for ContextError<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.context, f)
    }
}

impl<C: Display> Debug for ContextError<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextError")
            .field("context", &format_args!("{}", self.context))
            .field("error", &self.error)
            .finish()
    }
}

impl<C: Display> StdError for ContextError<C> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        // WHY: this one line is what links the layers into a chain.
        Some(&*self.error)
    }
}

impl Error {
    /// An error from a plain message.
    pub fn msg<M>(message: M) -> Error
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Error {
            inner: Box::new(MessageError(message)),
        }
    }

    /// Wraps `self` in a new outermost layer that displays as `context`.
    pub fn context<C>(self, context: C) -> Error
    where
        C: Display + Send + Sync + 'static,
    {
        Error {
            inner: Box::new(ContextError {
                context,
                error: self.inner,
            }),
        }
    }

    /// The outermost error first, then each `source()` in turn.
    pub fn chain(&self) -> impl Iterator<Item = &(dyn StdError + 'static)> {
        let first: &(dyn StdError + 'static) = &*self.inner;
        // WHY: `successors` hands the closure `&&dyn Error`. Pattern-match one `&`
        // away (`|&error|`) so `source()` borrows from the chain (lifetime of
        // `self`), not from the short-lived closure argument.
        std::iter::successors(Some(first), |&error| error.source())
    }

    /// The innermost error.
    pub fn root_cause(&self) -> &(dyn StdError + 'static) {
        self.chain()
            .last()
            .expect("a chain always has at least one element")
    }

    /// The first error in the chain that is an `E`.
    pub fn downcast_ref<E: StdError + 'static>(&self) -> Option<&E> {
        self.chain().find_map(|error| error.downcast_ref::<E>())
    }
}

/// `{}` shows the outermost message; `{:#}` shows the whole chain joined
/// with ": " (like anyhow).
impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)?;
        if f.alternate() {
            for cause in self.chain().skip(1) {
                write!(f, ": {cause}")?;
            }
        }
        Ok(())
    }
}

/// anyhow's `Debug` (what `fn main() -> Result<(), Error>` prints):
///
/// ```text
/// outer
///
/// Caused by:
///     0: middle
///     1: inner
/// ```
/// With exactly one cause the number is omitted (`    inner`). Messages are
/// assumed to be single-line.
impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)?;
        let causes: Vec<_> = self.chain().skip(1).collect();
        if !causes.is_empty() {
            write!(f, "\n\nCaused by:")?;
            let numbered = causes.len() > 1;
            for (n, cause) in causes.iter().enumerate() {
                if numbered {
                    // WHY: fmt.rs:84 — right-align the number in 5 columns.
                    write!(f, "\n{n: >5}: {cause}")?;
                } else {
                    write!(f, "\n    {cause}")?;
                }
            }
        }
        Ok(())
    }
}

/// `.context()` / `.with_context()` on `Result` and `Option`.
pub trait Context<T> {
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Debug + Send + Sync + 'static;

    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: Display + Debug + Send + Sync + 'static,
        F: FnOnce() -> C;
}

impl<T, E> Context<T> for Result<T, E>
where
    E: Into<Error>,
{
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Debug + Send + Sync + 'static,
    {
        self.map_err(|error| error.into().context(context))
    }

    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: Display + Debug + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        // WHY: the closure only runs on the error path, so building an
        // expensive message costs nothing when things succeed.
        self.map_err(|error| error.into().context(f()))
    }
}

impl<T> Context<T> for Option<T> {
    fn context<C>(self, context: C) -> Result<T, Error>
    where
        C: Display + Debug + Send + Sync + 'static,
    {
        self.ok_or_else(|| Error::msg(context))
    }

    fn with_context<C, F>(self, f: F) -> Result<T, Error>
    where
        C: Display + Debug + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.ok_or_else(|| Error::msg(f()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::num::ParseIntError;

    fn parse_port(s: &str) -> Result<u16, Error> {
        let port: u16 = s.parse()?; // `?` converts ParseIntError via From
        Ok(port)
    }

    fn load() -> Result<String, Error> {
        Err(io::Error::new(io::ErrorKind::NotFound, "vss.json missing")).context("reading VSS file")
    }

    #[test]
    fn question_mark_converts_any_std_error() {
        let err = parse_port("99999").expect_err("out of range");
        assert!(err.downcast_ref::<ParseIntError>().is_some());
        assert_eq!(err.to_string(), "number too large to fit in target type");
    }

    #[test]
    fn context_builds_a_chain() {
        let err = load().context("starting broker").expect_err("fails");
        let messages: Vec<String> = err.chain().map(|e| e.to_string()).collect();
        assert_eq!(
            messages,
            ["starting broker", "reading VSS file", "vss.json missing"]
        );
        assert_eq!(err.root_cause().to_string(), "vss.json missing");
        let io_err = err.downcast_ref::<io::Error>().expect("io error in chain");
        assert_eq!(io_err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn with_context_is_lazy_and_option_works() {
        let mut called = false;
        let ok: Result<u8, Error> = Ok::<u8, io::Error>(1).with_context(|| {
            called = true;
            "never built"
        });
        assert_eq!(ok.ok(), Some(1));
        assert!(!called);

        let err = None::<u8>.context("no such signal").expect_err("none");
        assert_eq!(err.to_string(), "no such signal");
        assert_eq!(err.chain().count(), 1);
    }

    #[test]
    fn display_and_alternate_display() {
        let err = load().context("starting broker").expect_err("fails");
        assert_eq!(format!("{err}"), "starting broker");
        assert_eq!(
            format!("{err:#}"),
            "starting broker: reading VSS file: vss.json missing"
        );
    }

    /// Real anyhow appends "Stack backtrace:" when RUST_BACKTRACE is set;
    /// compare only the part before it.
    fn without_backtrace(s: String) -> String {
        match s.find("\n\nStack backtrace:") {
            Some(i) => s[..i].to_string(),
            None => s,
        }
    }

    #[test]
    fn formats_exactly_like_real_anyhow() {
        // Both crates' `Context` traits are in scope here, so method syntax
        // would be ambiguous (E0034). Fully qualified calls pick one.
        let real = || -> anyhow::Result<()> {
            let io: Result<(), io::Error> =
                Err(io::Error::new(io::ErrorKind::NotFound, "vss.json missing"));
            let layer1 = anyhow::Context::context(io, "reading VSS file");
            anyhow::Context::context(layer1, "starting broker")
        };
        let mini =
            || -> Result<(), Error> { Context::context(load().map(drop), "starting broker") };
        let real = real().expect_err("fails");
        let mini = mini().expect_err("fails");

        assert_eq!(format!("{mini}"), format!("{real}"));
        assert_eq!(format!("{mini:#}"), format!("{real:#}"));
        assert_eq!(format!("{mini:?}"), without_backtrace(format!("{real:?}")));

        // One cause: no numbering.
        let real_one = anyhow::Error::msg("inner").context("outer");
        let mini_one = Error::msg("inner").context("outer");
        assert_eq!(
            format!("{mini_one:?}"),
            without_backtrace(format!("{real_one:?}"))
        );
        assert_eq!(format!("{mini_one:?}"), "outer\n\nCaused by:\n    inner");
    }
}
