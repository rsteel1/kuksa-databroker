// c31_e02_tracing_basics.rs
//
// CONCEPT: structured logging with `tracing` — levels, fields, spans, `#[instrument]`, EnvFilter.
// GUIDE:   chapters/31-building-a-service/guide.md  (section "Logging with tracing")
// DATABROKER: databroker/src/lib.rs — `init_logging` (EnvFilter from RUST_LOG, default "info");
//             broker.rs — `debug!`, `info!`, `warn!` calls and `#[cfg_attr(feature="otel", tracing::instrument(...))]`
// EXPECT:  tests fail: `todo!()` panics
//
// `tracing` separates *emitting* events (`info!(path, value, "value updated")`)
// from *deciding what to do with them* (a subscriber: print, filter, send to
// OpenTelemetry...). Events carry key=value fields, not just a string, so a
// log pipeline can search `path="Vehicle.Speed"` without regexes.
//
// A *span* is a period of time with its own fields; events inside it are
// tagged with it. `#[tracing::instrument]` wraps a whole function in a span
// named after the function, recording its arguments as fields
// (`skip(...)` leaves some out).
//
// The databroker reads the filter from the RUST_LOG environment variable
// (e.g. `RUST_LOG=databroker=debug`), falling back to "info".
//
// YOUR TASK
//   1. `filter_from`: build an EnvFilter from `spec`, falling back to "info"
//      when `spec` is None or invalid.
//   2. `apply_update`: add `#[tracing::instrument(skip(value))]`, and emit:
//        * `warn!` with field `path` and message "rejected negative value"
//          if value < 0.0 (then return false)
//        * `debug!` with fields `path`, `value` and message "value updated"
//          otherwise (then return true)
//   Do not edit the tests (they capture log output into a buffer).

// I AM NOT DONE

use tracing::{debug, warn};
use tracing_subscriber::EnvFilter;

fn filter_from(spec: Option<&str>) -> EnvFilter {
    todo!("EnvFilter::try_new(..) and a fallback of EnvFilter::new(\"info\")")
}

fn apply_update(path: &str, value: f32) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    /// A `Write` target shared between the subscriber and the test.
    #[derive(Clone, Default)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl Write for Buffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// Runs `f` with a subscriber using `filter`, returns everything logged.
    fn capture(filter: EnvFilter, f: impl FnOnce()) -> String {
        let buffer = Buffer::default();
        let writer = buffer.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, f);
        let bytes = buffer.0.lock().unwrap().clone();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn debug_events_hidden_at_info() {
        let out = capture(filter_from(None), || {
            assert!(apply_update("Vehicle.Speed", 10.0));
        });
        assert!(
            !out.contains("value updated"),
            "debug should be filtered: {out}"
        );
    }

    #[test]
    fn debug_events_shown_at_debug() {
        let out = capture(filter_from(Some("debug")), || {
            assert!(apply_update("Vehicle.Speed", 10.0));
        });
        assert!(out.contains("DEBUG"), "{out}");
        assert!(out.contains("value updated"), "{out}");
        assert!(
            out.contains("value=10"),
            "the value field is recorded: {out}"
        );
    }

    #[test]
    fn warnings_pass_the_default_filter_and_carry_the_span() {
        let out = capture(filter_from(None), || {
            assert!(!apply_update("Vehicle.Speed", -1.0));
        });
        assert!(out.contains("WARN"), "{out}");
        assert!(out.contains("rejected negative value"), "{out}");
        // the #[instrument] span shows up as `apply_update{path=...}`
        assert!(out.contains("apply_update{"), "missing span: {out}");
        assert!(out.contains("Vehicle.Speed"), "{out}");
    }

    #[test]
    fn invalid_filter_falls_back_to_info() {
        let out = capture(filter_from(Some("=[not a filter")), || {
            apply_update("Vehicle.Speed", 1.0);
            apply_update("Vehicle.Speed", -1.0);
        });
        assert!(!out.contains("value updated"), "{out}");
        assert!(out.contains("rejected negative value"), "{out}");
    }
}
