// c29_e06_review_glob_and_permission_edge_cases.rs
//
// CONCEPT: code review — this file COMPILES, but has 3 planted bugs in the
//          two hottest small functions on the databroker's request path: the
//          VSS-path-to-glob translation, its "branch matches itself" retry,
//          and which scopes imply read access.
// GUIDE:   chapters/29-strings-globs-permissions/guide.md  (section "VSS
//          wildcards and the Matcher type")
// DATABROKER: databroker/src/glob.rs:72-83 — `to_glob_string` appends "/**"
//             only when the pattern has NO wildcard at all (a plain branch
//             like "Vehicle.Cabin" must also match everything below it);
//             databroker/src/glob.rs:56-64 — `is_match` retries WITHOUT the
//             "/**" suffix when the first match fails, because `glob-match`
//             does not consider "A/B/**" to match "A/B" itself;
//             databroker/src/permissions.rs:167-176 — `can_read` is granted
//             if the path matches read OR actuate OR provide OR create.
// EXPECT:  tests fail: a plain branch pattern with no wildcard stops
//          matching its own children, an exact leaf pattern like
//          "Vehicle.Speed" stops matching the signal itself, and a
//          provide-only token can't read the signal it provides.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests.

use glob_match::glob_match;

/// Same translation as `glob::Matcher::to_glob_string`: `.` -> `/`, and a
/// pattern with no wildcard at all also covers everything below it.
fn to_glob_string(pattern: &str) -> String {
    let mut glob = pattern.replace('.', "/");
    // BUG 1: the suffix must be appended when the pattern has NO wildcard
    // (so a plain branch also covers its children); the condition was
    // inverted, so wildcard patterns got an extra unwanted "/**" and plain
    // branches got none at all.
    if !glob.contains('*') {
        glob.push_str("/**");
    }
    glob
}

/// Same retry as `glob::Matcher::is_match`: in `glob-match`, `A/B/**` does
/// not match `A/B` itself, so a second attempt strips the suffix.
fn is_match(glob: &str, path: &str) -> bool {
    let path = path.replace('.', "/");
    if glob_match(glob, &path) {
        return true;
    }
    if glob.ends_with("/**") {
        // BUG 2: the retry re-matched against the SAME glob (still ending in
        // "/**"), which is exactly what just failed. It must strip the
        // suffix first.
        let branch = glob.strip_suffix("/**").expect("checked ends_with above");
        return glob_match(branch, &path);
    }
    false
}

/// Same OR-list as `permissions::Permissions::can_read`.
fn can_read(
    read: &[&str],
    actuate: &[&str],
    provide: &[&str],
    create: &[&str],
    path: &str,
) -> bool {
    // BUG 3: `provide` was missing from this list, so a token that may only
    // `provide` a signal (write its sensor value) could not read it back,
    // even though read is implied by every other scope.
    [read, actuate, provide, create].iter().any(|globs| {
        globs
            .iter()
            .any(|pattern| is_match(&to_glob_string(pattern), path))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_wildcard_branch_pattern_still_matches_its_descendants() {
        let glob = to_glob_string("Vehicle.Cabin");
        assert!(
            is_match(&glob, "Vehicle.Cabin.Door.IsOpen"),
            "a plain branch pattern (no `*` anywhere) must match everything below it"
        );
    }

    #[test]
    fn exact_leaf_pattern_matches_the_signal_itself() {
        let glob = to_glob_string("Vehicle.Speed");
        assert!(
            is_match(&glob, "Vehicle.Speed"),
            "\"Vehicle.Speed\" must match the signal Vehicle.Speed, not just its (nonexistent) children"
        );
    }

    #[test]
    fn wildcard_pattern_is_untouched() {
        // Already has a `*`: to_glob_string must NOT append another "/**".
        assert_eq!(to_glob_string("Vehicle.Cabin.*"), "Vehicle/Cabin/*");
        assert!(is_match(
            &to_glob_string("Vehicle.Cabin.*"),
            "Vehicle.Cabin.Door"
        ));
        assert!(!is_match(
            &to_glob_string("Vehicle.Cabin.*"),
            "Vehicle.Cabin.Door.IsOpen"
        ));
    }

    #[test]
    fn can_read_is_implied_by_provide() {
        assert!(
            can_read(&[], &[], &["Vehicle.Speed"], &[], "Vehicle.Speed"),
            "provide implies read, by convention"
        );
        assert!(!can_read(
            &[],
            &[],
            &["Vehicle.Speed"],
            &[],
            "Vehicle.Width"
        ));
    }
}
