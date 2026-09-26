// c07_e06_panic_vs_result.rs
//
// CONCEPT: when to panic (bugs, broken startup invariants) vs return Result;
//          `expect` over `unwrap`; `anyhow` + `.context()` in application code.
// GUIDE:   chapters/07-errors/guide.md  (section "Panic, expect, or Result?")
// DATABROKER: databroker/src/main.rs — `shutdown_handler` uses
//             `.expect("failed to setup SIGINT signal handler")` (nothing
//             sensible to do if that fails at startup), while
//             `read_metadata_file` returns `Result<(), Box<dyn Error>>` so
//             `main` can report a bad VSS file instead of crashing mid-request.
// EXPECT:  tests fail: `todo!()` panics
//
// Rule of thumb:
//   * Expected failure the caller can handle (bad input, missing signal,
//     network) -> Result.
//   * A bug, or a startup precondition without which the program can't
//     run at all -> panic, via `expect("why this must hold")`.
//   * Library code: typed errors (thiserror). Binaries: `anyhow::Result`
//     with `.context("what we were doing")` for readable error chains.
//
// YOUR TASK
//   Implement the three functions as documented. Do not edit the tests.

use anyhow::{bail, Context};

/// Startup: the port comes from an environment variable that the deployment
/// guarantees. If it's missing or invalid the service must not start at all.
/// Panic with a message containing "KUKSA_PORT must be a valid port".
fn port_from_env(value: Option<&str>) -> u16 {
    value
        .and_then(|v| v.parse().ok())
        .expect("KUKSA_PORT must be a valid port")
}

/// Runtime: a client sends a buffer size. Bad input must NOT crash the
/// broker — return an error instead. Valid range: 0..=1000 (like the
/// databroker's MAX_SUBSCRIBE_BUFFER_SIZE).
fn buffer_size(requested: &str) -> Result<usize, String> {
    let n: usize = requested
        .parse()
        .map_err(|_| format!("buffer size is not a number: {requested}"))?;
    if n > 1000 {
        return Err(format!("buffer size {n} exceeds maximum 1000"));
    }
    Ok(n)
}

#[derive(Debug, PartialEq)]
struct Config {
    port: u16,
    vss_files: Vec<String>,
}

/// Application code: parse a config line "port=55555;vss=a.json,b.json"
/// using `anyhow`. Each failure gets context so the final message explains
/// what was being done:
///   - missing "port=" part   -> bail!("missing port")
///   - bad port number        -> context "invalid port `<text>`"
///   - no vss files           -> bail!("no VSS files configured")
fn parse_config(line: &str) -> anyhow::Result<Config> {
    let mut port = None;
    let mut vss_files = Vec::new();
    for part in line.split(';') {
        match part.split_once('=') {
            Some(("port", text)) => {
                let parsed: u16 = text
                    .parse()
                    .with_context(|| format!("invalid port `{text}`"))?;
                port = Some(parsed);
            }
            Some(("vss", files)) => {
                vss_files = files
                    .split(',')
                    .filter(|f| !f.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            _ => {}
        }
    }
    let Some(port) = port else {
        bail!("missing port");
    };
    if vss_files.is_empty() {
        bail!("no VSS files configured");
    }
    Ok(Config { port, vss_files })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_port_ok() {
        assert_eq!(port_from_env(Some("55555")), 55555);
    }

    #[test]
    #[should_panic(expected = "KUKSA_PORT must be a valid port")]
    fn env_port_missing_panics() {
        port_from_env(None);
    }

    #[test]
    #[should_panic(expected = "KUKSA_PORT must be a valid port")]
    fn env_port_garbage_panics() {
        port_from_env(Some("99999"));
    }

    #[test]
    fn buffer_size_is_a_result() {
        assert_eq!(buffer_size("10"), Ok(10));
        assert_eq!(buffer_size("1000"), Ok(1000));
        assert_eq!(
            buffer_size("1001"),
            Err("buffer size 1001 exceeds maximum 1000".to_string())
        );
        assert_eq!(
            buffer_size("ten"),
            Err("buffer size is not a number: ten".to_string())
        );
    }

    #[test]
    fn config_ok() {
        assert_eq!(
            parse_config("port=55555;vss=a.json,b.json").unwrap(),
            Config {
                port: 55555,
                vss_files: vec!["a.json".into(), "b.json".into()]
            }
        );
    }

    #[test]
    fn config_errors_have_context() {
        let err = parse_config("vss=a.json").unwrap_err();
        assert_eq!(err.to_string(), "missing port");

        let err = parse_config("port=http;vss=a.json").unwrap_err();
        // `{:#}` prints the whole chain: context, then the underlying cause.
        assert_eq!(
            format!("{err:#}"),
            "invalid port `http`: invalid digit found in string"
        );

        let err = parse_config("port=1;vss=").unwrap_err();
        assert_eq!(err.to_string(), "no VSS files configured");
    }
}
