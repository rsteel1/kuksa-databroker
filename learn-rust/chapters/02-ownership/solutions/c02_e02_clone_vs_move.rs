// c02_e02_clone_vs_move.rs
//
// CONCEPT: `.clone()` — when two independent owners are genuinely needed.
// GUIDE:   chapters/02-ownership/guide.md  (section "Clone: paying for a second copy")
// DATABROKER: databroker/src/broker.rs — `AuthorizedAccess::get_datapoint`
//             returns `entry.datapoint.clone()`: the database keeps its copy,
//             the caller gets its own, so the lock can be released.
// EXPECT:  compile errors E0382 (use of moved value: `path`) and E0507
//          (cannot move out of `registry.paths`, which is behind a reference).
//
// Sometimes you really need the data in two places that will live and change
// independently. Then you pay for a copy with `.clone()`. Cloning a String
// allocates new heap memory and copies the bytes, so do it on purpose, not to
// silence the compiler.
//
// YOUR TASK
//   1. `register` must store the path in BOTH `registry.paths` and
//      `registry.audit_log` (as "registered <path>"), and return the new id.
//      Fix the E0382 WITHOUT cloning: `format!` only needs to *read* `path`,
//      so do things in the right order.
//   2. `snapshot` must return an independent copy of the paths. Here a
//      `.clone()` is the right tool — explain to yourself why.
//   Do not edit the tests.

struct Registry {
    paths: Vec<String>,
    audit_log: Vec<String>,
}

fn register(registry: &mut Registry, path: String) -> usize {
    // WHY: `format!` only borrows `path`, producing a brand-new String.
    // Doing it before the move means no clone is needed.
    registry.audit_log.push(format!("registered {path}"));
    registry.paths.push(path);
    registry.paths.len() - 1
}

/// Returns an owned copy of all registered paths. The registry must keep its
/// own list, and the caller wants a list it can change freely: two
/// independent owners, so here a clone is exactly right.
fn snapshot(registry: &Registry) -> Vec<String> {
    // WHY: we only have `&Registry` (a loan) — we can't move `paths` out of
    // something we merely borrowed (E0507). Clone makes a second owner.
    registry.paths.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_path_and_log() {
        let mut registry = Registry {
            paths: Vec::new(),
            audit_log: Vec::new(),
        };
        let id = register(&mut registry, String::from("Vehicle.Speed"));
        assert_eq!(id, 0);
        assert_eq!(registry.paths, vec!["Vehicle.Speed"]);
        assert_eq!(registry.audit_log, vec!["registered Vehicle.Speed"]);
    }

    #[test]
    fn snapshot_is_independent() {
        let mut registry = Registry {
            paths: Vec::new(),
            audit_log: Vec::new(),
        };
        register(&mut registry, String::from("Vehicle.Width"));
        let mut copy = snapshot(&registry);
        copy.push(String::from("Vehicle.Height"));
        assert_eq!(registry.paths, vec!["Vehicle.Width"]);
        assert_eq!(copy, vec!["Vehicle.Width", "Vehicle.Height"]);
    }
}
