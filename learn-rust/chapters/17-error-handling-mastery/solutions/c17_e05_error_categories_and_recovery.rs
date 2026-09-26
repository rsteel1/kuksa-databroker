// c17_e05_error_categories_and_recovery.rs
//
// CONCEPT: classify every error by *who handles it* and *how to recover*.
//          User-facing errors get a clear, safe message. Internal errors get
//          a generic message outside and full detail in the logs. Transient
//          errors may be retried or answered from a fallback; permanent ones
//          (bad input, permission denied) must never be masked by one.
// GUIDE:   chapters/17-error-handling-mastery/guide.md  (section "Who is this error for? Categories and recovery")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/conversions.rs:356 `to_status_with_code`
//             maps each domain error to a gRPC code, and broker.rs:2294
//             `get_values_broker` falls back to the database when a provider
//             fails, but a *permission* failure is returned, not papered over.
// EXPECT:  tests fail: `todo!()` panics
//
// YOUR TASK
//   Implement `category`, `is_retryable`, `to_status`, `log_line` and
//   `with_fallback`. Do not edit the tests.

use std::error::Error;

/// A cut-down set of gRPC status codes (the real ones are `tonic::Code`,
/// which you'll meet in chapter 30).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Code {
    InvalidArgument,
    NotFound,
    PermissionDenied,
    Unavailable,
    DeadlineExceeded,
    Internal,
}

#[derive(Debug, PartialEq, Eq)]
struct Status {
    code: Code,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    /// Caused by the request; the client must change something.
    UserFacing,
    /// Might succeed if tried again later.
    Transient,
    /// A bug or broken environment on *our* side.
    Internal,
}

#[derive(Debug, thiserror::Error)]
enum BrokerError {
    #[error("invalid argument: {0}")]
    InvalidInput(String),
    #[error("signal not found: {0}")]
    NotFound(String),
    #[error("permission denied for {0}")]
    PermissionDenied(String),
    #[error("provider for {0} is unavailable")]
    ProviderUnavailable(String),
    #[error("provider for {0} timed out")]
    ProviderTimeout(String),
    #[error("internal error")]
    Internal(#[source] Box<dyn Error + Send + Sync>),
}

impl BrokerError {
    /// InvalidInput / NotFound / PermissionDenied -> UserFacing,
    /// ProviderUnavailable / ProviderTimeout -> Transient,
    /// Internal -> Internal.
    fn category(&self) -> Category {
        match self {
            BrokerError::InvalidInput(_)
            | BrokerError::NotFound(_)
            | BrokerError::PermissionDenied(_) => Category::UserFacing,
            BrokerError::ProviderUnavailable(_) | BrokerError::ProviderTimeout(_) => {
                Category::Transient
            }
            BrokerError::Internal(_) => Category::Internal,
        }
    }

    /// Only transient errors are worth retrying.
    fn is_retryable(&self) -> bool {
        self.category() == Category::Transient
    }

    /// What the *client* sees. Codes: InvalidInput -> InvalidArgument,
    /// NotFound -> NotFound, PermissionDenied -> PermissionDenied,
    /// ProviderUnavailable -> Unavailable, ProviderTimeout -> DeadlineExceeded,
    /// Internal -> Internal.
    /// Message: the error's own `Display` text, which never includes the
    /// source. For `Internal` that's just "internal error": the source may
    /// contain secrets, file paths or SQL and must not leak to clients.
    fn to_status(&self) -> Status {
        let code = match self {
            BrokerError::InvalidInput(_) => Code::InvalidArgument,
            BrokerError::NotFound(_) => Code::NotFound,
            BrokerError::PermissionDenied(_) => Code::PermissionDenied,
            BrokerError::ProviderUnavailable(_) => Code::Unavailable,
            BrokerError::ProviderTimeout(_) => Code::DeadlineExceeded,
            BrokerError::Internal(_) => Code::Internal,
        };
        Status {
            code,
            message: self.to_string(),
        }
    }

    /// What goes to *our* logs: the full chain, formatted as
    /// "[<category:?>] msg: cause: cause ..." (each source appended after ": ").
    fn log_line(&self) -> String {
        let mut line = format!("[{:?}] {self}", self.category());
        let mut source = self.source();
        while let Some(cause) = source {
            line.push_str(": ");
            line.push_str(&cause.to_string());
            source = cause.source();
        }
        line
    }
}

/// Graceful degradation, like `get_values_broker`: try the live provider;
/// if (and only if) it fails with a *retryable* error, answer from the
/// fallback (the last known value in the database). Any other error is
/// returned as-is: never hide a PermissionDenied behind cached data!
fn with_fallback<T>(
    primary: impl FnOnce() -> Result<T, BrokerError>,
    fallback: impl FnOnce() -> Result<T, BrokerError>,
) -> Result<T, BrokerError> {
    match primary() {
        Err(e) if e.is_retryable() => fallback(),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn internal(detail: &str) -> BrokerError {
        BrokerError::Internal(detail.to_string().into())
    }

    #[test]
    fn categories() {
        assert_eq!(
            BrokerError::NotFound("x".into()).category(),
            Category::UserFacing
        );
        assert_eq!(
            BrokerError::InvalidInput("x".into()).category(),
            Category::UserFacing
        );
        assert_eq!(
            BrokerError::PermissionDenied("x".into()).category(),
            Category::UserFacing
        );
        assert_eq!(
            BrokerError::ProviderTimeout("x".into()).category(),
            Category::Transient
        );
        assert_eq!(internal("boom").category(), Category::Internal);
    }

    #[test]
    fn only_transient_is_retryable() {
        assert!(BrokerError::ProviderUnavailable("Vehicle.Speed".into()).is_retryable());
        assert!(BrokerError::ProviderTimeout("Vehicle.Speed".into()).is_retryable());
        assert!(!BrokerError::PermissionDenied("Vehicle.Speed".into()).is_retryable());
        assert!(!BrokerError::InvalidInput("bad".into()).is_retryable());
        assert!(!internal("boom").is_retryable());
    }

    #[test]
    fn status_mapping() {
        assert_eq!(
            BrokerError::NotFound("Vehicle.Nope".into()).to_status(),
            Status {
                code: Code::NotFound,
                message: "signal not found: Vehicle.Nope".into()
            }
        );
        assert_eq!(
            BrokerError::ProviderTimeout("Vehicle.Speed".into())
                .to_status()
                .code,
            Code::DeadlineExceeded
        );
        assert_eq!(
            BrokerError::InvalidInput("x".into()).to_status().code,
            Code::InvalidArgument
        );
    }

    #[test]
    fn internals_never_leak_to_clients_but_reach_the_logs() {
        let err = internal("db connect failed: password=hunter2");
        let status = err.to_status();
        assert_eq!(status.code, Code::Internal);
        assert!(!status.message.contains("hunter2"), "leaked: {status:?}");
        assert_eq!(
            err.log_line(),
            "[Internal] internal error: db connect failed: password=hunter2"
        );
        assert_eq!(
            BrokerError::NotFound("A".into()).log_line(),
            "[UserFacing] signal not found: A"
        );
    }

    #[test]
    fn fallback_only_for_transient_errors() {
        let got = with_fallback(
            || Err(BrokerError::ProviderTimeout("Vehicle.Speed".into())),
            || Ok(42),
        );
        assert_eq!(got.unwrap(), 42);

        let got = with_fallback(|| Ok(7), || -> Result<i32, _> { panic!("not needed") });
        assert_eq!(got.unwrap(), 7);

        let got = with_fallback(
            || -> Result<i32, _> { Err(BrokerError::PermissionDenied("Vehicle.Speed".into())) },
            || Ok(42),
        );
        assert!(matches!(got, Err(BrokerError::PermissionDenied(_))));
    }
}
