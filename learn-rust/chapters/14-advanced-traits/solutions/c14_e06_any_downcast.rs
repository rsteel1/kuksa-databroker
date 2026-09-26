// c14_e06_any_downcast.rs
//
// CONCEPT: `dyn Any` and downcasting — recovering a concrete type from a
//          type-erased value at runtime; trait upcasting `&dyn Plugin` ->
//          `&dyn Any`.
// GUIDE:   chapters/14-advanced-traits/guide.md  (section "Any and downcasting")
// DATABROKER: databroker-proto/src/lib.rs — imports `std::any::Any` and, in the
//             error message of `Value::new`, prints `dt.type_id()`. That is
//             `Any::type_id`: an opaque `TypeId` like `TypeId(0x1f…)`, not the
//             variant name the author probably wanted (`{:?}` of `dt` would be).
//             Knowing what `Any` really gives you lets you spot that.
// EXPECT:  tests fail: `todo!()` panics
//
// `Any` is implemented for every `'static` type. A `&dyn Any` has forgotten
// its concrete type, but `downcast_ref::<T>()` asks "are you a T?" and hands
// back `Option<&T>`. Since Rust 1.86 a `&dyn Plugin` (where `Plugin: Any`)
// coerces to `&dyn Any` directly ("trait upcasting"); older code adds an
// `fn as_any(&self) -> &dyn Any` method to every trait for the same effect.
//
// YOUR TASK
//   1. `describe`: "i32 7", "string hello" (for String AND &'static str),
//      otherwise "unknown".
//   2. `find_plugin::<T>`: return the first plugin whose concrete type is T.
//   Do not edit the tests.

use std::any::Any;

fn describe(value: &dyn Any) -> String {
    if let Some(n) = value.downcast_ref::<i32>() {
        format!("i32 {n}")
    } else if let Some(s) = value.downcast_ref::<String>() {
        format!("string {s}")
    } else if let Some(s) = value.downcast_ref::<&'static str>() {
        format!("string {s}")
    } else {
        "unknown".to_string()
    }
}

/// Plugins are stored type-erased; `Any` as a supertrait lets us recover them.
trait Plugin: Any {
    fn name(&self) -> &str;
}

struct CanBridge {
    bus: &'static str,
}

struct Logger;

impl Plugin for CanBridge {
    fn name(&self) -> &str {
        "can-bridge"
    }
}

impl Plugin for Logger {
    fn name(&self) -> &str {
        "logger"
    }
}

fn find_plugin<T: Plugin>(plugins: &[Box<dyn Plugin>]) -> Option<&T> {
    plugins.iter().find_map(|plugin| {
        // WHY: trait upcasting — `&dyn Plugin` becomes `&dyn Any` because
        // `Any` is a supertrait of `Plugin`.
        let any: &dyn Any = plugin.as_ref();
        any.downcast_ref::<T>()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describe_values() {
        assert_eq!(describe(&7_i32), "i32 7");
        assert_eq!(describe(&String::from("hello")), "string hello");
        assert_eq!(describe(&"hello"), "string hello");
        assert_eq!(describe(&1.5_f64), "unknown");
    }

    #[test]
    fn downcast_plugins() {
        let plugins: Vec<Box<dyn Plugin>> =
            vec![Box::new(Logger), Box::new(CanBridge { bus: "can0" })];
        let bridge = find_plugin::<CanBridge>(&plugins).expect("bridge registered");
        assert_eq!(bridge.bus, "can0");
        assert_eq!(bridge.name(), "can-bridge");
        assert!(find_plugin::<Logger>(&plugins).is_some());

        let only_logger: Vec<Box<dyn Plugin>> = vec![Box::new(Logger)];
        assert!(find_plugin::<CanBridge>(&only_logger).is_none());
    }
}
