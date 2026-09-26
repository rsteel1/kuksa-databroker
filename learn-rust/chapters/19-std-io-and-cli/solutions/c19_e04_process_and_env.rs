// c19_e04_process_and_env.rs
//
// CONCEPT: `std::process::Command` (spawn, capture output, exit status,
//          stdin pipes, per-child environment), reading configuration from
//          environment variables testably, and exit codes.
// GUIDE:   chapters/19-std-io-and-cli/guide.md  (section "Processes, environment and exit codes")
// DATABROKER: databroker/src/main.rs — `main` returns
//             `Result<(), Box<dyn std::error::Error>>` (line 232), so an `Err`
//             prints the error and exits with code 1; every clap argument
//             has `.env("KUKSA_DATABROKER_...")` so it can come from the
//             environment (e.g. line 286).
// EXPECT:  tests fail: `todo!()` panics.
//
// The tests only use `sh`, `printf`, `tr` and `exit`, which exist on every
// Linux (including the course devcontainer) and macOS.
//
// YOUR TASK
//   Implement the `todo!()`s:
//     - `run_capture`: run `program` with `args`, return its stdout with
//       trailing whitespace trimmed. A program that can't be started →
//       `RunError::Spawn`; a non-zero exit → `RunError::Failed` with the exit
//       code (`None` if killed by a signal) and the trimmed stderr;
//     - `run_with_stdin`: run `sh -c <script>`, write `input` to its stdin,
//       close stdin, and return the trimmed stdout;
//     - `child_env`: run `sh -c 'printf %s "$<key>"'` with `key=value` set in
//       the *child's* environment only, return what it printed;
//     - `port_from_env`: read `KUKSA_DATABROKER_PORT` through the injected
//       `lookup` function (default 55555; invalid → Err naming the value);
//     - `exit_code`: map a `CliError` to a process exit code (Usage → 2,
//       Io → 74, the BSD sysexits `EX_IOERR`; Other → 1).
//   Do not edit the tests.

use std::io::{self, Write};
use std::process::{Command, Stdio};

#[derive(Debug)]
enum RunError {
    Spawn(io::Error),
    Failed { code: Option<i32>, stderr: String },
}

fn run_capture(program: &str, args: &[&str]) -> Result<String, RunError> {
    // WHY: `output()` waits for the child and captures stdout AND stderr
    // (unlike `status()`, which inherits the parent's terminal).
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(RunError::Spawn)?;
    if !output.status.success() {
        return Err(RunError::Failed {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr)
                .trim_end()
                .to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

fn run_with_stdin(script: &str, input: &str) -> io::Result<String> {
    let mut child = Command::new("sh")
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    // WHY: `take()` moves the pipe out of the child; dropping it at the end
    // of this block closes stdin, so the child sees end-of-input. Without
    // that, `tr` would wait for more input forever.
    {
        let mut stdin = child.stdin.take().expect("stdin was piped");
        stdin.write_all(input.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

fn child_env(key: &str, value: &str) -> io::Result<String> {
    let script = format!("printf %s \"${key}\"");
    let output = Command::new("sh")
        .args(["-c", &script])
        .env(key, value)
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// `lookup` is `|k| std::env::var(k).ok()` in production and a map in tests.
fn port_from_env(lookup: impl Fn(&str) -> Option<String>) -> Result<u16, String> {
    match lookup("KUKSA_DATABROKER_PORT") {
        None => Ok(55555),
        Some(raw) => raw
            .trim()
            .parse()
            .map_err(|_| format!("KUKSA_DATABROKER_PORT is not a valid port: {raw:?}")),
    }
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Io(io::Error),
    Other(String),
}

fn exit_code(error: &CliError) -> u8 {
    match error {
        CliError::Usage(_) => 2,
        CliError::Io(_) => 74,
        CliError::Other(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn captures_stdout() {
        assert_eq!(
            run_capture("printf", &["%s\\n", "Vehicle.Speed"]).unwrap(),
            "Vehicle.Speed"
        );
    }

    #[test]
    fn non_zero_exit_is_an_error_with_stderr() {
        match run_capture("sh", &["-c", "echo boom >&2; exit 3"]) {
            Err(RunError::Failed { code, stderr }) => {
                assert_eq!(code, Some(3));
                assert_eq!(stderr, "boom");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn missing_program_is_a_spawn_error() {
        match run_capture("definitely-not-a-real-program-c19", &[]) {
            Err(RunError::Spawn(e)) => assert_eq!(e.kind(), io::ErrorKind::NotFound),
            other => panic!("expected Spawn, got {other:?}"),
        }
    }

    #[test]
    fn pipes_stdin() {
        assert_eq!(
            run_with_stdin("tr a-z A-Z", "vehicle.speed\n").unwrap(),
            "VEHICLE.SPEED"
        );
    }

    #[test]
    fn env_is_set_for_the_child_only() {
        assert_eq!(
            child_env("C19_TEST_VAR", "from-parent").unwrap(),
            "from-parent"
        );
        assert!(std::env::var("C19_TEST_VAR").is_err());
    }

    #[test]
    fn port_configuration() {
        let env: HashMap<&str, &str> = HashMap::new();
        assert_eq!(
            port_from_env(|k| env.get(k).map(|v| v.to_string())),
            Ok(55555)
        );

        let env = HashMap::from([("KUKSA_DATABROKER_PORT", " 55556 ")]);
        assert_eq!(
            port_from_env(|k| env.get(k).map(|v| v.to_string())),
            Ok(55556)
        );

        let env = HashMap::from([("KUKSA_DATABROKER_PORT", "99999")]);
        let err = port_from_env(|k| env.get(k).map(|v| v.to_string())).unwrap_err();
        assert!(
            err.contains("\"99999\""),
            "error should name the value: {err}"
        );
    }

    #[test]
    fn exit_codes() {
        assert_eq!(exit_code(&CliError::Usage("bad flag".into())), 2);
        assert_eq!(exit_code(&CliError::Io(io::Error::other("disk"))), 74);
        assert_eq!(exit_code(&CliError::Other("x".into())), 1);
    }
}
