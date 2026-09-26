// c08_e02_use_paths_and_reexports.rs
//
// CONCEPT: paths (`crate::`, `super::`, `self::`), `use`, and re-exports (`pub use`).
// GUIDE:   chapters/08-modules-and-cargo/guide.md  (section "Paths, use and re-exports")
// DATABROKER: databroker/src/broker.rs lines 16-18:
//               pub use crate::types;
//               pub use crate::types::{ChangeType, DataType, DataValue, ...};
//             so callers can write `broker::DataValue` instead of
//             `databroker::types::DataValue`.
// EXPECT:  compile errors E0432 (unresolved import) / E0433 (failed to resolve)
//
// Since the 2018 edition a `use` path inside a module starts from *that
// module*, not from the crate root. To reach a sibling module you write
// `crate::sibling::Item` (absolute, from the crate root) or
// `super::sibling::Item` (relative, from the parent).
//
// `pub use path::Item;` re-exports: `Item` becomes reachable at the new
// location too — it's how a crate shapes its public API independently of
// its file layout.
//
// YOUR TASK
//   1. Fix the broken `use` lines inside `grpc` (use `crate::` for one and
//      `super::` for the other so you've tried both).
//   2. Add re-exports to `broker` so the tests can use `broker::DataType`,
//      `broker::DataValue` and `broker::types::...`.
//   Do not edit the tests.

// I AM NOT DONE

// `pub` here matters: a re-export can't be more visible than the item it
// re-exports (try removing it later and read error E0365).
pub mod types {
    #[derive(Debug, Clone, PartialEq)]
    pub enum DataType {
        Int32,
        String,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum DataValue {
        Int32(i32),
        String(String),
    }
}

mod broker {
    // TODO: re-export the `types` module and its two enums here.

    pub fn type_of(value: &DataValue) -> DataType {
        match value {
            DataValue::Int32(_) => DataType::Int32,
            DataValue::String(_) => DataType::String,
        }
    }
}

mod grpc {
    use broker::type_of;
    use types::DataValue;

    /// Name used on the wire for the value's type.
    pub fn wire_type_name(value: &DataValue) -> &'static str {
        // `self::` is "this module" — optional here, shown for completeness.
        self::name_of(type_of(value))
    }

    fn name_of(t: crate::types::DataType) -> &'static str {
        match t {
            crate::types::DataType::Int32 => "int32",
            crate::types::DataType::String => "string",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::broker;
    use super::grpc;

    #[test]
    fn reexports_work() {
        let v = broker::DataValue::Int32(5);
        assert_eq!(broker::type_of(&v), broker::DataType::Int32);
        // The module itself is re-exported too:
        let s = broker::types::DataValue::String("x".into());
        assert_eq!(broker::type_of(&s), broker::types::DataType::String);
    }

    #[test]
    fn grpc_uses_both_paths() {
        assert_eq!(grpc::wire_type_name(&broker::DataValue::Int32(1)), "int32");
        assert_eq!(
            grpc::wire_type_name(&broker::DataValue::String("a".into())),
            "string"
        );
    }
}
