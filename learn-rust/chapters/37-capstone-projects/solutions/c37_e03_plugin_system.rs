// c37_e03_plugin_system.rs
//
// CONCEPT: PROJECT, no scaffolding. Runtime extensibility with trait
//          objects: a registry of factories, config-driven instantiation
//          with complete error reporting, and fault isolation so one bad
//          plugin can't take down the host.
// GUIDE:   chapters/37-capstone-projects/guide.md  (section "Project 3: a plugin host")
// DATABROKER: databroker/src/broker.rs — providers are runtime "plugins":
//             `Arc<dyn SignalProvider>` registered at runtime, removed by
//             housekeeping when they fail (`cleanup_signal_providers_subscriptions`),
//             and every call to them is bounded so a bad one can't block the rest.
// EXPECT:  tests fail: `todo!()` panics
//
// THE SPEC
//   `Plugin` and `Factory` are fixed (the tests implement them).
//
//   Host::register_kind(kind, factory)
//       Adds a plugin kind. Registering the same kind twice is
//       `HostError::DuplicateKind(kind)`.
//   Host::load_config(text)
//       One instance per line:  <name> = <kind> [key=value ...]
//       Blank lines and lines starting with `#` are ignored. Whitespace
//       around tokens is free. Report EVERY problem (line numbers from 1):
//         HostError::Syntax { line }                 not `<name> = <kind> key=value...`
//         HostError::UnknownKind { line, kind }
//         HostError::DuplicateName { line, name }    (also vs. already-loaded instances)
//         HostError::Factory { line, name, message } the factory returned Err(message)
//       All or nothing: if there is any error, no instance from this text
//       is added.
//   Host::publish(path, value) -> Vec<Event>
//       Calls `on_update` on every ACTIVE instance, in name order, and
//       returns the events in that order:
//         Ok(messages)  -> one Event::Output { plugin, message } per message
//         Err(error)    -> Event::Failed { plugin, error }; after 3
//                          CONSECUTIVE failures also Event::Disabled { plugin }
//                          and the instance is disabled (a success resets the count)
//         panic         -> Event::Panicked { plugin, message } then
//                          Event::Disabled { plugin }; the instance is disabled at
//                          once (its state may be corrupt). The host keeps going.
//   Host::status(name) -> Option<Status>   Active or Disabled.
//
// YOUR TASK
//   Design and implement `Host`. Keep the public items' names and
//   signatures; add anything private. Do not edit the tests.

use std::collections::{BTreeMap, HashMap};
use std::panic::{catch_unwind, AssertUnwindSafe};

pub trait Plugin: Send {
    fn on_update(&mut self, path: &str, value: f64) -> Result<Vec<String>, String>;
}

pub type Config = BTreeMap<String, String>;
pub type Factory = Box<dyn Fn(&Config) -> Result<Box<dyn Plugin>, String> + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub enum HostError {
    DuplicateKind(String),
    Syntax {
        line: usize,
    },
    UnknownKind {
        line: usize,
        kind: String,
    },
    DuplicateName {
        line: usize,
        name: String,
    },
    Factory {
        line: usize,
        name: String,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Output { plugin: String, message: String },
    Failed { plugin: String, error: String },
    Panicked { plugin: String, message: String },
    Disabled { plugin: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    Active,
    Disabled,
}

const MAX_CONSECUTIVE_FAILURES: u32 = 3;

struct Instance {
    plugin: Box<dyn Plugin>,
    failures: u32,
    status: Status,
}

#[derive(Default)]
pub struct Host {
    kinds: HashMap<String, Factory>,
    // WHY: BTreeMap gives "name order" for free.
    instances: BTreeMap<String, Instance>,
}

impl Host {
    pub fn new() -> Host {
        Host::default()
    }

    pub fn register_kind(&mut self, kind: &str, factory: Factory) -> Result<(), HostError> {
        if self.kinds.contains_key(kind) {
            return Err(HostError::DuplicateKind(kind.to_string()));
        }
        self.kinds.insert(kind.to_string(), factory);
        Ok(())
    }

    pub fn load_config(&mut self, text: &str) -> Result<(), Vec<HostError>> {
        let mut errors = Vec::new();
        let mut staged: BTreeMap<String, Instance> = BTreeMap::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let raw = raw.trim();
            if raw.is_empty() || raw.starts_with('#') {
                continue;
            }
            let Some((name, kind, config)) = parse_line(raw) else {
                errors.push(HostError::Syntax { line });
                continue;
            };
            if self.instances.contains_key(name) || staged.contains_key(name) {
                errors.push(HostError::DuplicateName {
                    line,
                    name: name.to_string(),
                });
                continue;
            }
            let Some(factory) = self.kinds.get(kind) else {
                errors.push(HostError::UnknownKind {
                    line,
                    kind: kind.to_string(),
                });
                continue;
            };
            match factory(&config) {
                Ok(plugin) => {
                    staged.insert(
                        name.to_string(),
                        Instance {
                            plugin,
                            failures: 0,
                            status: Status::Active,
                        },
                    );
                }
                Err(message) => errors.push(HostError::Factory {
                    line,
                    name: name.to_string(),
                    message,
                }),
            }
        }
        if errors.is_empty() {
            self.instances.append(&mut staged);
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn publish(&mut self, path: &str, value: f64) -> Vec<Event> {
        let mut events = Vec::new();
        for (name, instance) in &mut self.instances {
            if instance.status == Status::Disabled {
                continue;
            }
            // WHY: AssertUnwindSafe is our promise that we won't observe the
            // plugin's possibly half-updated state after a panic, and we
            // keep it by disabling the instance immediately.
            let outcome = catch_unwind(AssertUnwindSafe(|| instance.plugin.on_update(path, value)));
            match outcome {
                Ok(Ok(messages)) => {
                    instance.failures = 0;
                    events.extend(messages.into_iter().map(|message| Event::Output {
                        plugin: name.clone(),
                        message,
                    }));
                }
                Ok(Err(error)) => {
                    instance.failures += 1;
                    events.push(Event::Failed {
                        plugin: name.clone(),
                        error,
                    });
                    if instance.failures >= MAX_CONSECUTIVE_FAILURES {
                        instance.status = Status::Disabled;
                        events.push(Event::Disabled {
                            plugin: name.clone(),
                        });
                    }
                }
                Err(payload) => {
                    instance.status = Status::Disabled;
                    events.push(Event::Panicked {
                        plugin: name.clone(),
                        message: panic_message(payload.as_ref()),
                    });
                    events.push(Event::Disabled {
                        plugin: name.clone(),
                    });
                }
            }
        }
        events
    }

    pub fn status(&self, name: &str) -> Option<Status> {
        self.instances.get(name).map(|instance| instance.status)
    }
}

fn parse_line(line: &str) -> Option<(&str, &str, Config)> {
    let (name, rest) = line.split_once('=')?;
    let name = name.trim();
    if name.is_empty() || name.contains(char::is_whitespace) {
        return None;
    }
    let mut tokens = rest.split_whitespace();
    let kind = tokens.next()?;
    let mut config = Config::new();
    for token in tokens {
        let (key, value) = token.split_once('=')?;
        if key.is_empty() {
            return None;
        }
        config.insert(key.to_string(), value.to_string());
    }
    Some((name, kind, config))
}

/// A panic payload is usually a `&'static str` (`panic!("literal")`) or a
/// `String` (`panic!("{x}")`); anything else gets a generic message.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "unknown panic".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Emits "<path> above <limit>" whenever value > limit.
    struct Threshold {
        limit: f64,
    }

    impl Plugin for Threshold {
        fn on_update(&mut self, path: &str, value: f64) -> Result<Vec<String>, String> {
            Ok(if value > self.limit {
                vec![format!("{path} above {}", self.limit)]
            } else {
                vec![]
            })
        }
    }

    /// Counts updates; fails on negative values; panics on NaN.
    struct Fragile {
        seen: u32,
    }

    impl Plugin for Fragile {
        fn on_update(&mut self, _path: &str, value: f64) -> Result<Vec<String>, String> {
            assert!(!value.is_nan(), "cannot handle NaN");
            if value < 0.0 {
                return Err(format!("negative value {value}"));
            }
            self.seen += 1;
            Ok(vec![format!("seen {}", self.seen)])
        }
    }

    fn host() -> Host {
        let mut host = Host::new();
        host.register_kind(
            "threshold",
            Box::new(|config: &Config| {
                let limit = config
                    .get("limit")
                    .ok_or("missing limit")?
                    .parse::<f64>()
                    .map_err(|e| format!("bad limit: {e}"))?;
                Ok(Box::new(Threshold { limit }) as Box<dyn Plugin>)
            }),
        )
        .expect("first registration");
        host.register_kind(
            "fragile",
            Box::new(|_: &Config| Ok(Box::new(Fragile { seen: 0 }) as Box<dyn Plugin>)),
        )
        .expect("first registration");
        host
    }

    fn output(plugin: &str, message: &str) -> Event {
        Event::Output {
            plugin: plugin.into(),
            message: message.into(),
        }
    }

    #[test]
    fn kinds_are_unique() {
        let mut host = host();
        let again = host.register_kind("fragile", Box::new(|_: &Config| Err("unused".to_string())));
        assert_eq!(again, Err(HostError::DuplicateKind("fragile".into())));
    }

    #[test]
    fn config_creates_instances_and_publish_runs_them_in_name_order() {
        let mut host = host();
        let config = "\
# alarms
speed_alarm = threshold limit=120
  counter=fragile

a_low = threshold   limit=10
";
        host.load_config(config).expect("valid config");
        assert_eq!(host.status("speed_alarm"), Some(Status::Active));
        assert_eq!(host.status("nope"), None);
        assert_eq!(
            host.publish("Vehicle.Speed", 130.0),
            vec![
                output("a_low", "Vehicle.Speed above 10"),
                output("counter", "seen 1"),
                output("speed_alarm", "Vehicle.Speed above 120"),
            ]
        );
        assert_eq!(
            host.publish("Vehicle.Speed", 5.0),
            vec![output("counter", "seen 2")]
        );
    }

    #[test]
    fn config_errors_are_all_reported_and_nothing_is_loaded() {
        let mut host = host();
        host.load_config("existing = fragile").expect("valid");
        let config = "\
ok = threshold limit=1
this line is wrong
x = warp_drive
existing = fragile
ok = fragile
t = threshold limit=fast
y = threshold =3
";
        let errors = host.load_config(config).expect_err("config has errors");
        assert_eq!(
            errors,
            vec![
                HostError::Syntax { line: 2 },
                HostError::UnknownKind {
                    line: 3,
                    kind: "warp_drive".into()
                },
                HostError::DuplicateName {
                    line: 4,
                    name: "existing".into()
                },
                HostError::DuplicateName {
                    line: 5,
                    name: "ok".into()
                },
                HostError::Factory {
                    line: 6,
                    name: "t".into(),
                    message: "bad limit: invalid float literal".into()
                },
                HostError::Syntax { line: 7 },
            ]
        );
        assert_eq!(host.status("ok"), None, "a failed load must add nothing");
        assert_eq!(host.status("existing"), Some(Status::Active));
    }

    #[test]
    fn three_consecutive_failures_disable_a_plugin() {
        let mut host = host();
        host.load_config("f = fragile\nt = threshold limit=0")
            .expect("valid");
        let failed = |error: &str| Event::Failed {
            plugin: "f".into(),
            error: error.into(),
        };
        assert_eq!(host.publish("p", -1.0), vec![failed("negative value -1")]);
        assert_eq!(host.publish("p", -2.0), vec![failed("negative value -2")]);
        // A success resets the count.
        assert_eq!(
            host.publish("p", 1.0),
            vec![output("f", "seen 1"), output("t", "p above 0")]
        );
        host.publish("p", -1.0);
        host.publish("p", -1.0);
        assert_eq!(
            host.publish("p", -3.0),
            vec![
                failed("negative value -3"),
                Event::Disabled { plugin: "f".into() }
            ]
        );
        assert_eq!(host.status("f"), Some(Status::Disabled));
        assert_eq!(host.publish("p", 2.0), vec![output("t", "p above 0")]);
    }

    #[test]
    fn a_panicking_plugin_is_isolated_and_disabled() {
        let mut host = host();
        host.load_config("a = fragile\nb = threshold limit=0")
            .expect("valid");
        let events = host.publish("p", f64::NAN);
        assert_eq!(
            events,
            vec![
                Event::Panicked {
                    plugin: "a".into(),
                    message: "cannot handle NaN".into()
                },
                Event::Disabled { plugin: "a".into() },
            ],
            "b must still run (NaN > 0 is false, so it emits nothing)"
        );
        assert_eq!(host.status("a"), Some(Status::Disabled));
        assert_eq!(host.publish("p", 1.0), vec![output("b", "p above 0")]);
    }

    #[test]
    fn host_can_move_to_another_thread() {
        fn assert_send<T: Send>() {}
        assert_send::<Host>();
    }
}
