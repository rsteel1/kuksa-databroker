// c17_e09_review_context_and_from.rs
//
// CONCEPT: code review — this file COMPILES and looks reasonable, but has
//          3 planted bugs typical of chapter 17 (a blanket `From` impl that
//          maps every parse error to the wrong variant, a swallowed error
//          that silently defaults instead of propagating, and a missing
//          check that lets an empty field masquerade as a parse error).
// GUIDE:   chapters/17-error-handling-mastery/guide.md  (section "Error chains: source() and context")
// DATABROKER: databroker/src/broker.rs / conversions.rs — configuration and
//             request parsing needs the *right* error variant with the
//             original cause attached, so operators can tell which field
//             was bad instead of getting a generic, misleading message.
// EXPECT:  tests fail: wrong error variant, a silently-defaulted value, and
//          a missing field reported as a parse error instead of itself.
//
// A small config loader turns three string fields into a `Settings`.
// `ConfigError` is supposed to say exactly which field failed and why.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

use thiserror::Error;

#[derive(Debug, Error)]
enum ConfigError {
    #[error("invalid port")]
    InvalidPort(#[source] std::num::ParseIntError),
    #[error("invalid timeout_ms")]
    InvalidTimeout(#[source] std::num::ParseIntError),
    #[error("invalid retries")]
    InvalidRetries(#[source] std::num::ParseIntError),
    #[error("missing field: {0}")]
    MissingField(String),
}

// A single blanket conversion used by every `?` below, regardless of which
// field is actually being parsed.
impl From<std::num::ParseIntError> for ConfigError {
    fn from(e: std::num::ParseIntError) -> Self {
        ConfigError::InvalidTimeout(e)
    }
}

fn parse_port(raw: &str) -> Result<u16, ConfigError> {
    Ok(raw.parse::<u16>()?)
}

fn parse_timeout_ms(raw: &str) -> Result<u64, ConfigError> {
    Ok(raw.parse::<u64>()?)
}

fn parse_retries(raw: &str) -> Result<u32, ConfigError> {
    Ok(raw.parse::<u32>().unwrap_or(0))
}

#[derive(Debug, PartialEq)]
struct Settings {
    port: u16,
    timeout_ms: u64,
    retries: u32,
}

fn load_settings(port: &str, timeout: &str, retries: &str) -> Result<Settings, ConfigError> {
    let port = parse_port(port)?;
    let timeout_ms = parse_timeout_ms(timeout)?;
    let retries = parse_retries(retries)?;
    Ok(Settings {
        port,
        timeout_ms,
        retries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_port_reports_the_port_variant() {
        let err = parse_port("not-a-number").unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidPort(_)),
            "expected InvalidPort, got {err:?}"
        );
    }

    #[test]
    fn bad_retries_is_an_error_not_a_silent_default() {
        let err = load_settings("80", "1000", "not-a-number").unwrap_err();
        assert!(matches!(err, ConfigError::InvalidRetries(_)));
    }

    #[test]
    fn empty_field_reports_missing_not_a_parse_failure() {
        let err = load_settings("", "1000", "3").unwrap_err();
        assert!(matches!(err, ConfigError::MissingField(ref f) if f == "port"));
    }

    #[test]
    fn valid_input_still_loads() {
        let s = load_settings("8090", "5000", "3").unwrap();
        assert_eq!(
            s,
            Settings {
                port: 8090,
                timeout_ms: 5000,
                retries: 3
            }
        );
    }
}
