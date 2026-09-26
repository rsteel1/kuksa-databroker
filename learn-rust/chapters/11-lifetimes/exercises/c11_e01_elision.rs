// c11_e01_elision.rs
//
// CONCEPT: lifetime elision — when the compiler can infer lifetimes on its
//          own, and when it needs you to say which input a returned
//          reference borrows from.
// GUIDE:   chapters/11-lifetimes/guide.md  (section "Elision: the rules you already rely on")
// DATABROKER: databroker/src/broker.rs — `EntryReadAccess::metadata(&self) -> &Metadata`
//          relies on elision; `Database::authorized_read_access<'a, 'b>`
//          can't, because it has two reference inputs.
// EXPECT:  compile error E0106 (missing lifetime specifier) — twice.
//
// A returned reference must point at *something the caller gave you*.
// The elision rules let you skip writing lifetimes when there is only one
// possible source:
//   1. each reference parameter gets its own lifetime;
//   2. if there is exactly ONE input lifetime, outputs get it;
//   3. if one of the inputs is `&self`/`&mut self`, outputs get self's.
// With two plain reference inputs and no self, the compiler refuses to guess.
//
// YOUR TASK
//   Add explicit lifetime parameters to `longer_path` and `pick_path` so the
//   file compiles and the tests pass. Use descriptive names such as `'p`
//   rather than `'a` if you like. `first_segment` and `Signal::path` compile
//   already — understand why. Do not edit the tests.

// I AM NOT DONE

/// Rule 2: one input reference, so the output borrows from it.
fn first_segment(path: &str) -> &str {
    path.split('.').next().unwrap_or(path)
}

struct Signal {
    path: String,
}

impl Signal {
    /// Rule 3: the output borrows from `self`.
    fn path(&self) -> &str {
        &self.path
    }
}

/// Returns whichever path is longer (the first one on a tie).
fn longer_path(a: &str, b: &str) -> &str {
    if b.len() > a.len() {
        b
    } else {
        a
    }
}

/// Returns the first path that is not empty, or `fallback`.
/// The result may come from either argument.
fn pick_path(paths: &[String], fallback: &str) -> &str {
    paths
        .iter()
        .find(|p| !p.is_empty())
        .map(|p| p.as_str())
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elided_cases() {
        assert_eq!(first_segment("Vehicle.Speed"), "Vehicle");
        let s = Signal {
            path: "Vehicle.Speed".into(),
        };
        assert_eq!(s.path(), "Vehicle.Speed");
    }

    #[test]
    fn longer() {
        assert_eq!(longer_path("Vehicle.Speed", "A.B"), "Vehicle.Speed");
        assert_eq!(longer_path("A.B", "Vehicle.Speed"), "Vehicle.Speed");
    }

    #[test]
    fn pick() {
        let paths = vec![String::new(), "Vehicle.Horn".to_string()];
        assert_eq!(pick_path(&paths, "Vehicle"), "Vehicle.Horn");
        assert_eq!(pick_path(&[], "Vehicle"), "Vehicle");
    }
}
