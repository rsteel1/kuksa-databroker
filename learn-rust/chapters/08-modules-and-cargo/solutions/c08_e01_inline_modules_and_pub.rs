// c08_e01_inline_modules_and_pub.rs
//
// CONCEPT: modules (`mod`) and `pub`: everything is private by default.
// GUIDE:   chapters/08-modules-and-cargo/guide.md  (section "Modules and privacy")
// DATABROKER: databroker/src/lib.rs — `pub mod broker; pub mod glob; ...`;
//             databroker/src/broker.rs — `pub struct DataBroker` whose fields
//             (`database`, `subscriptions`, ...) are private, exposed only
//             through methods like `get_version()`.
// EXPECT:  compile errors E0603 (struct `DataBroker` is private, module `types`
//          is private, ...), then — once those are fixed — E0624 (method is private).
//
// A module is a namespace *and* a privacy boundary. Items (fns, structs,
// modules, consts...) are private to their parent module unless marked
// `pub`. Struct *fields* are private too, even in a `pub struct` — that's how
// a type protects its invariants: nobody outside can build it with
// arbitrary field values or poke at its internals.
//
// YOUR TASK
//   Add `pub` where needed so the tests compile — and ONLY where needed.
//   The struct fields must stay private (use the methods). The helper
//   `sanitize` must stay private. Do not edit the tests.

mod broker {
    pub mod types {
        // WHY: variants of a `pub enum` are automatically public.
        #[derive(Debug, PartialEq)]
        pub enum DataType {
            Bool,
            Int32,
            String,
        }
    }

    pub struct DataBroker {
        version: String,
        commit_sha: String,
    }

    impl DataBroker {
        pub fn new(version: &str, commit_sha: &str) -> Self {
            DataBroker {
                version: sanitize(version),
                commit_sha: sanitize(commit_sha),
            }
        }

        pub fn version(&self) -> &str {
            &self.version
        }

        pub fn commit_sha(&self) -> &str {
            &self.commit_sha
        }

        pub fn type_of(&self, path: &str) -> Option<types::DataType> {
            match path {
                "Vehicle.Speed" => Some(types::DataType::Int32),
                "Vehicle.IsMoving" => Some(types::DataType::Bool),
                "Vehicle.VehicleIdentification.VIN" => Some(types::DataType::String),
                _ => None,
            }
        }
    }

    /// Private helper: trims whitespace. Callers outside `broker` never need it.
    fn sanitize(s: &str) -> String {
        s.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::broker::types::DataType;
    use super::broker::DataBroker;

    #[test]
    fn construct_through_new_and_read_through_methods() {
        let b = DataBroker::new(" 0.7.2 ", "abc123\n");
        assert_eq!(b.version(), "0.7.2");
        assert_eq!(b.commit_sha(), "abc123");
    }

    #[test]
    fn nested_module_types_are_reachable() {
        let b = DataBroker::new("1", "x");
        assert_eq!(b.type_of("Vehicle.Speed"), Some(DataType::Int32));
        assert_eq!(b.type_of("Vehicle.IsMoving"), Some(DataType::Bool));
        assert_eq!(b.type_of("Nope"), None);
    }
}
