// c19_e06_checkpoint_part2.rs
//
// CONCEPT: CHECKPOINT for Part II: a config-file loader combining layered
//          errors (ch. 17), a declarative macro (ch. 18), `FromStr` (ch. 06),
//          a typestate builder (ch. 14) and file I/O (ch. 19).
// GUIDE:   chapters/19-std-io-and-cli/guide.md  (section "Checkpoint")
// DATABROKER: databroker/src/main.rs — the databroker's configuration comes
//             from clap arguments and `KUKSA_DATABROKER_*` environment
//             variables (lines 262-412); this is the config-*file* version.
// EXPECT:  compile errors: `Settings` doesn't exist yet (the macro expands to
//          nothing) and `ConfigError` doesn't implement `Display`/`Error`.
//
// Config file format (one setting per line, `#` starts a comment line):
//
//     # databroker.conf
//     address = 0.0.0.0
//     port = 55556
//     log_level = debug
//     insecure = true
//     vss = data/vss_4.0.json      <- repeatable, at least one required
//     vss = overlay.json
//
// YOUR TASK (signatures, types and tests are given; the design is yours)
//   1. `config_with_defaults!`: expand the invocation below into the struct
//      *and* an `impl Default` that uses the `= value` defaults.
//   2. `LogLevel: FromStr`, case-insensitive; error `ParseLogLevelError`
//      displays as "unknown log level 'loud'".
//   3. `ConfigError`: implement `Display` and `std::error::Error` (with
//      `source()` for Io and InvalidValue) — `thiserror` is the easy way. The
//      messages, exactly:
//        Io            "cannot read config file <path>"   (source: io::Error)
//        Syntax        "line <n>: expected `key = value`, got \"<text>\""
//        InvalidValue  "line <n>: invalid value for '<key>'" (source: parse error)
//        UnknownKey    "line <n>: unknown key '<key>'"
//        Missing       "missing required key '<key>'"
//   4. The typestate `ConfigBuilder`: `build()` must only exist once at least
//      one VSS file was added (`ConfigBuilder<HasVss>`).
//   5. `parse_config`, `load_config` and `error_chain` ("outer: inner: ...").
//   Do not edit the tests.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, PartialEq)]
struct ParseLogLevelError(String);

impl fmt::Display for ParseLogLevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown log level '{}'", self.0)
    }
}

impl Error for ParseLogLevelError {}

impl FromStr for LogLevel {
    type Err = ParseLogLevelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "error" => Ok(LogLevel::Error),
            "warn" => Ok(LogLevel::Warn),
            "info" => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            "trace" => Ok(LogLevel::Trace),
            _ => Err(ParseLogLevelError(s.to_string())),
        }
    }
}

macro_rules! config_with_defaults {
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $($field:ident : $ty:ty = $default:expr),* $(,)?
        }
    ) => {
        $(#[$meta])*
        struct $name {
            $($field: $ty),*
        }

        impl Default for $name {
            fn default() -> Self {
                $name {
                    $($field: $default),*
                }
            }
        }
    };
}

config_with_defaults! {
    #[derive(Debug, Clone, PartialEq)]
    struct Settings {
        address: String = "127.0.0.1".to_string(),
        port: u16 = 55555,
        log_level: LogLevel = LogLevel::Info,
        insecure: bool = false,
    }
}

#[derive(Debug, thiserror::Error)]
enum ConfigError {
    #[error("cannot read config file {}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("line {line}: expected `key = value`, got {text:?}")]
    Syntax { line: usize, text: String },
    #[error("line {line}: invalid value for '{key}'")]
    InvalidValue {
        line: usize,
        key: String,
        // WHY: port, bool and log-level parsing fail with three different
        // error types; a boxed trait object keeps them all as the `source`.
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    #[error("line {line}: unknown key '{key}'")]
    UnknownKey { line: usize, key: String },
    #[error("missing required key '{0}'")]
    Missing(&'static str),
}

#[derive(Debug, PartialEq)]
struct Config {
    settings: Settings,
    vss_files: Vec<PathBuf>,
}

/// Typestate markers: has at least one VSS file been added?
struct NoVss;
struct HasVss;

struct ConfigBuilder<State> {
    settings: Settings,
    vss_files: Vec<PathBuf>,
    _state: PhantomData<State>,
}

impl ConfigBuilder<NoVss> {
    fn new() -> Self {
        ConfigBuilder {
            settings: Settings::default(),
            vss_files: Vec::new(),
            _state: PhantomData,
        }
    }
}

impl<State> ConfigBuilder<State> {
    fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    /// Adding a file always moves the builder into the `HasVss` state.
    fn vss_file(mut self, path: impl Into<PathBuf>) -> ConfigBuilder<HasVss> {
        self.vss_files.push(path.into());
        ConfigBuilder {
            settings: self.settings,
            vss_files: self.vss_files,
            _state: PhantomData,
        }
    }
}

impl ConfigBuilder<HasVss> {
    fn build(self) -> Config {
        Config {
            settings: self.settings,
            vss_files: self.vss_files,
        }
    }
}

fn parse_config(text: &str) -> Result<Config, ConfigError> {
    let mut settings = Settings::default();
    let mut vss_files: Vec<PathBuf> = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            return Err(ConfigError::Syntax {
                line,
                text: trimmed.to_string(),
            });
        };
        let (key, value) = (key.trim(), value.trim());
        // WHY: each parser fails with its own error type; `Into::into` boxes
        // them all into one type, so the arms agree and a single `map_err`
        // attaches the line number and key.
        let applied: Result<(), Box<dyn Error + Send + Sync>> = match key {
            "address" => {
                settings.address = value.to_string();
                Ok(())
            }
            "port" => value.parse().map(|v| settings.port = v).map_err(Into::into),
            "log_level" => value
                .parse()
                .map(|v| settings.log_level = v)
                .map_err(Into::into),
            "insecure" => value
                .parse()
                .map(|v| settings.insecure = v)
                .map_err(Into::into),
            "vss" => {
                vss_files.push(PathBuf::from(value));
                Ok(())
            }
            _ => {
                return Err(ConfigError::UnknownKey {
                    line,
                    key: key.to_string(),
                })
            }
        };
        applied.map_err(|source| ConfigError::InvalidValue {
            line,
            key: key.to_string(),
            source,
        })?;
    }

    // WHY: the typestate builder only offers `build()` after `vss_file`, so
    // "no VSS file" must be handled here, explicitly, before building.
    let Some((first, rest)) = vss_files.split_first() else {
        return Err(ConfigError::Missing("vss"));
    };
    let builder = rest
        .iter()
        .fold(ConfigBuilder::new().vss_file(first), |b, path| {
            b.vss_file(path)
        });
    Ok(builder.settings(settings).build())
}

fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let text = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_config(&text)
}

/// "outer: cause: root cause", walking `Error::source()`.
fn error_chain(error: &dyn Error) -> String {
    let mut out = error.to_string();
    let mut current = error.source();
    while let Some(cause) = current {
        out.push_str(": ");
        out.push_str(&cause.to_string());
        current = cause.source();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = "\
# databroker.conf
address = 0.0.0.0
port = 55556

log_level = DEBUG
insecure = true
vss = data/vss_4.0.json
vss = overlay.json
";

    #[test]
    fn defaults_come_from_the_macro() {
        let s = Settings::default();
        assert_eq!(s.address, "127.0.0.1");
        assert_eq!(s.port, 55555);
        assert_eq!(s.log_level, LogLevel::Info);
        assert!(!s.insecure);
    }

    #[test]
    fn log_level_from_str() {
        assert_eq!("Trace".parse::<LogLevel>(), Ok(LogLevel::Trace));
        let err = "loud".parse::<LogLevel>().unwrap_err();
        assert_eq!(err.to_string(), "unknown log level 'loud'");
    }

    #[test]
    fn typestate_builder() {
        let config = ConfigBuilder::new()
            .vss_file("a.json")
            .vss_file(PathBuf::from("b.json"))
            .build();
        assert_eq!(
            config.vss_files,
            vec![PathBuf::from("a.json"), PathBuf::from("b.json")]
        );
        assert_eq!(config.settings, Settings::default());
        // `ConfigBuilder::new().build()` must NOT compile: try it!
    }

    #[test]
    fn parses_a_full_file() {
        let config = parse_config(FULL).unwrap();
        assert_eq!(
            config,
            Config {
                settings: Settings {
                    address: "0.0.0.0".into(),
                    port: 55556,
                    log_level: LogLevel::Debug,
                    insecure: true,
                },
                vss_files: vec!["data/vss_4.0.json".into(), "overlay.json".into()],
            }
        );
    }

    #[test]
    fn only_vss_is_required() {
        let config = parse_config("vss = v.json").unwrap();
        assert_eq!(config.settings, Settings::default());
    }

    #[test]
    fn syntax_error() {
        let err = parse_config("# c\nport = 1\nthis is not a setting\n").unwrap_err();
        assert_eq!(
            err.to_string(),
            "line 3: expected `key = value`, got \"this is not a setting\""
        );
        assert!(err.source().is_none());
    }

    #[test]
    fn invalid_values_keep_their_cause() {
        let err = parse_config("vss = a\nport = 99999\n").unwrap_err();
        assert_eq!(
            error_chain(&err),
            "line 2: invalid value for 'port': number too large to fit in target type"
        );
        let err = parse_config("log_level = loud\nvss = a").unwrap_err();
        assert_eq!(
            error_chain(&err),
            "line 1: invalid value for 'log_level': unknown log level 'loud'"
        );
        // The source can be downcast back to its concrete type.
        let source = err.source().unwrap();
        assert_eq!(
            source.downcast_ref::<ParseLogLevelError>(),
            Some(&ParseLogLevelError("loud".into()))
        );
        let err = parse_config("insecure = yes\nvss = a").unwrap_err();
        assert!(error_chain(&err).starts_with("line 1: invalid value for 'insecure': "));
    }

    #[test]
    fn unknown_and_missing_keys() {
        let err = parse_config("vss = a\ncolour = red").unwrap_err();
        assert_eq!(err.to_string(), "line 2: unknown key 'colour'");
        let err = parse_config("port = 1").unwrap_err();
        assert_eq!(err.to_string(), "missing required key 'vss'");
    }

    #[test]
    fn io_errors_name_the_file() {
        let missing = Path::new("/no/such/dir/databroker.conf");
        let err = load_config(missing).unwrap_err();
        assert_eq!(
            err.to_string(),
            "cannot read config file /no/such/dir/databroker.conf"
        );
        let io = err.source().unwrap().downcast_ref::<io::Error>().unwrap();
        assert_eq!(io.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn loads_from_disk() {
        let path =
            std::env::temp_dir().join(format!("learn-rust-c19e06-{}.conf", std::process::id()));
        fs::write(&path, FULL).unwrap();
        let result = load_config(&path);
        fs::remove_file(&path).unwrap();
        assert_eq!(result.unwrap().settings.port, 55556);
    }
}
