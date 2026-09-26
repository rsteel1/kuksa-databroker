// c29_e05_permissions_can_read.rs
//
// CONCEPT: putting it together — JWT scope string -> Permissions -> access checks.
// GUIDE:   chapters/29-strings-globs-permissions/guide.md  (section "From a JWT scope to an access decision")
// DATABROKER: databroker/src/authorization/jwt/scope.rs — `parse_whitespace_separated`;
//             databroker/src/authorization/jwt/decoder.rs — `impl TryFrom<Claims> for Permissions`;
//             databroker/src/permissions.rs — `can_read`, `can_write_datapoint`, `is_expired`
// EXPECT:  tests fail: `todo!()` panics
//
// A client sends a JWT whose "scope" claim looks like:
//
//     "read:Vehicle.Cabin.* provide:Vehicle.Speed actuate"
//
// i.e. whitespace-separated `action[:path]` items. No path means "all paths".
// Actions: read, actuate, provide, create. Anything else is an error.
//
// Access rules (exactly the databroker's):
//   * Every check first fails with `Expired` if `expires_at` is in the past.
//   * READ is allowed if the path matches read OR actuate OR provide OR create
//     ("read permission is included, by convention, in the other permissions").
//   * writing a sensor value (`can_write_datapoint`) needs PROVIDE.
//   * setting an actuator target (`can_write_actuator_target`) needs ACTUATE.
//   * Otherwise `Denied`.
//
// A glob-matching helper (the rules from c29_e03) is provided.
//
// YOUR TASK
//   Implement `Permissions::from_scope`, `is_expired`, `can_read`,
//   `can_write_datapoint`, `can_write_actuator_target`.
//   Do not edit the tests.

// I AM NOT DONE

use glob_match::glob_match;
use std::time::{Duration, SystemTime};

/// VSS glob `pattern` matches dotted `path` (see c29_e03 for the rules).
fn vss_glob_matches(pattern: &str, path: &str) -> bool {
    let mut glob = pattern.replace('.', "/");
    if !pattern.contains('*') {
        glob.push_str("/**");
    }
    let path = path.replace('.', "/");
    glob_match(&glob, &path)
        || glob
            .strip_suffix("/**")
            .is_some_and(|branch| glob_match(branch, &path))
}

#[derive(Debug, Clone, PartialEq)]
enum PathMatcher {
    Nothing,
    Everything,
    Globs(Vec<String>),
}

impl PathMatcher {
    fn is_match(&self, path: &str) -> bool {
        match self {
            PathMatcher::Nothing => false,
            PathMatcher::Everything => true,
            PathMatcher::Globs(globs) => globs.iter().any(|g| vss_glob_matches(g, path)),
        }
    }

    fn allow(&mut self, path: Option<&str>) {
        match path {
            // No path: this action is allowed everywhere.
            None => *self = PathMatcher::Everything,
            Some(p) => match self {
                PathMatcher::Nothing => *self = PathMatcher::Globs(vec![p.to_string()]),
                PathMatcher::Everything => {}
                PathMatcher::Globs(globs) => globs.push(p.to_string()),
            },
        }
    }
}

#[derive(Debug, PartialEq)]
enum PermissionError {
    Denied,
    Expired,
}

#[derive(Debug, PartialEq)]
enum ScopeError {
    UnknownAction(String),
}

#[derive(Debug, Clone)]
struct Permissions {
    expires_at: Option<SystemTime>,
    read: PathMatcher,
    actuate: PathMatcher,
    provide: PathMatcher,
    create: PathMatcher,
}

impl Permissions {
    fn from_scope(scope: &str, expires_at: Option<SystemTime>) -> Result<Self, ScopeError> {
        todo!("split_whitespace, split_once(':'), match the action, call .allow(path)")
    }

    fn is_expired(&self) -> bool {
        todo!()
    }

    fn can_read(&self, path: &str) -> Result<(), PermissionError> {
        todo!()
    }

    fn can_write_datapoint(&self, path: &str) -> Result<(), PermissionError> {
        todo!()
    }

    fn can_write_actuator_target(&self, path: &str) -> Result<(), PermissionError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_one_hour() -> Option<SystemTime> {
        Some(SystemTime::now() + Duration::from_secs(3600))
    }

    #[test]
    fn parses_scopes() {
        let p = Permissions::from_scope("read:Vehicle.Cabin.* provide:Vehicle.Speed actuate", None)
            .unwrap();
        assert_eq!(p.read, PathMatcher::Globs(vec!["Vehicle.Cabin.*".into()]));
        assert_eq!(p.provide, PathMatcher::Globs(vec!["Vehicle.Speed".into()]));
        assert_eq!(p.actuate, PathMatcher::Everything);
        assert_eq!(p.create, PathMatcher::Nothing);
    }

    #[test]
    fn unknown_action_is_rejected() {
        let err = Permissions::from_scope("read delete:Vehicle", None).unwrap_err();
        assert_eq!(err, ScopeError::UnknownAction("delete".into()));
    }

    #[test]
    fn read_is_implied_by_other_scopes() {
        let p = Permissions::from_scope("provide:Vehicle.Speed", in_one_hour()).unwrap();
        assert_eq!(p.can_read("Vehicle.Speed"), Ok(()));
        assert_eq!(p.can_read("Vehicle.Width"), Err(PermissionError::Denied));
    }

    #[test]
    fn write_needs_the_specific_scope() {
        let p = Permissions::from_scope(
            "read provide:Vehicle.Speed actuate:Vehicle.Cabin.Door.**",
            in_one_hour(),
        )
        .unwrap();
        assert_eq!(p.can_write_datapoint("Vehicle.Speed"), Ok(()));
        assert_eq!(
            p.can_write_datapoint("Vehicle.Width"),
            Err(PermissionError::Denied)
        );
        assert_eq!(
            p.can_write_actuator_target("Vehicle.Cabin.Door.Row1.IsOpen"),
            Ok(())
        );
        assert_eq!(
            p.can_write_actuator_target("Vehicle.Speed"),
            Err(PermissionError::Denied),
            "provide does not grant actuate"
        );
        assert_eq!(p.can_read("Kuksa.Databroker.CargoVersion"), Ok(()));
    }

    #[test]
    fn expiry_wins_over_everything() {
        let past = Some(SystemTime::now() - Duration::from_secs(1));
        let p = Permissions::from_scope("read provide actuate create", past).unwrap();
        assert!(p.is_expired());
        assert_eq!(p.can_read("Vehicle.Speed"), Err(PermissionError::Expired));
        assert_eq!(
            p.can_write_datapoint("Vehicle.Speed"),
            Err(PermissionError::Expired)
        );
        assert_eq!(
            p.can_write_actuator_target("Vehicle.Speed"),
            Err(PermissionError::Expired)
        );
    }

    #[test]
    fn no_expiry_never_expires() {
        let p = Permissions::from_scope("read", None).unwrap();
        assert!(!p.is_expired());
        assert_eq!(p.can_read("Vehicle.Speed"), Ok(()));
    }
}
