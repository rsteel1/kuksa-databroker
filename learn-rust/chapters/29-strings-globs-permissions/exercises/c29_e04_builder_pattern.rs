// c29_e04_builder_pattern.rs
//
// CONCEPT: the consuming builder pattern (`fn add_x(mut self, ..) -> Self`).
// GUIDE:   chapters/29-strings-globs-permissions/guide.md  (section "The builder pattern")
// DATABROKER: databroker/src/permissions.rs — `PermissionBuilder`, `PathMatchBuilder::extend_with_glob`,
//             `PermissionBuilder::build`
// EXPECT:  tests fail: `todo!()` panics
//
// A `Permissions` value has several parts (read / provide scopes, expiry).
// The databroker assembles it from a JWT's scope list with a builder:
//
//     Permissions::builder()
//         .add_read_permission(Permission::Glob("Vehicle.Speed".into()))
//         .add_provide_permission(Permission::All)
//         .expires_at(exp)
//         .build()?
//
// Each method takes `mut self` BY VALUE, changes it, and returns it — so calls
// chain without any `&mut` juggling, and the half-built value can't be used by
// accident after `build()` (it has been moved).
//
// Each scope is a small state machine, `PathSet`:
//     Nothing --glob--> Globs([..]) --glob--> Globs([.., ..])
//        \--All--> Everything  (absorbing: nothing can shrink it again)
//
// `build()` is the single place that validates. A glob is valid when it is
// non-empty and contains no whitespace; otherwise `build` returns
// `Err(BuildError::InvalidGlob(<the glob>))`.
//
// YOUR TASK
//   Implement `PathSet::add` and every `PermissionBuilder` method.
//   Do not edit the tests.

// I AM NOT DONE

use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
enum Permission {
    Nothing,
    All,
    Glob(String),
}

#[derive(Debug, Clone, PartialEq)]
enum PathSet {
    Nothing,
    Everything,
    Globs(Vec<String>),
}

impl PathSet {
    fn add(&mut self, permission: Permission) {
        todo!("follow the state machine in the header")
    }
}

#[derive(Debug, PartialEq)]
struct Permissions {
    expires_at: Option<SystemTime>,
    read: PathSet,
    provide: PathSet,
}

#[derive(Debug, PartialEq)]
enum BuildError {
    InvalidGlob(String),
}

struct PermissionBuilder {
    expires_at: Option<SystemTime>,
    read: PathSet,
    provide: PathSet,
}

impl Permissions {
    fn builder() -> PermissionBuilder {
        PermissionBuilder::new()
    }
}

impl PermissionBuilder {
    fn new() -> Self {
        todo!()
    }

    fn expires_at(mut self, when: SystemTime) -> Self {
        todo!()
    }

    fn add_read_permission(mut self, permission: Permission) -> Self {
        todo!()
    }

    fn add_provide_permission(mut self, permission: Permission) -> Self {
        todo!()
    }

    fn build(self) -> Result<Permissions, BuildError> {
        todo!("validate every glob in read and provide, then assemble Permissions")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_builder_allows_nothing() {
        let p = Permissions::builder().build().unwrap();
        assert_eq!(
            p,
            Permissions {
                expires_at: None,
                read: PathSet::Nothing,
                provide: PathSet::Nothing
            }
        );
    }

    #[test]
    fn chaining_globs() {
        let exp = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let p = Permissions::builder()
            .add_read_permission(Permission::Glob("Vehicle.Speed".into()))
            .add_read_permission(Permission::Glob("Vehicle.Cabin.*".into()))
            .add_provide_permission(Permission::Glob("Vehicle.Speed".into()))
            .expires_at(exp)
            .build()
            .unwrap();
        assert_eq!(p.expires_at, Some(exp));
        assert_eq!(
            p.read,
            PathSet::Globs(vec!["Vehicle.Speed".into(), "Vehicle.Cabin.*".into()])
        );
        assert_eq!(p.provide, PathSet::Globs(vec!["Vehicle.Speed".into()]));
    }

    #[test]
    fn all_is_absorbing() {
        let p = Permissions::builder()
            .add_read_permission(Permission::Glob("Vehicle.Speed".into()))
            .add_read_permission(Permission::All)
            .add_read_permission(Permission::Glob("Vehicle.Width".into()))
            .add_read_permission(Permission::Nothing)
            .build()
            .unwrap();
        assert_eq!(p.read, PathSet::Everything);
        assert_eq!(p.provide, PathSet::Nothing);
    }

    #[test]
    fn nothing_changes_nothing() {
        let p = Permissions::builder()
            .add_provide_permission(Permission::Nothing)
            .build()
            .unwrap();
        assert_eq!(p.provide, PathSet::Nothing);
    }

    #[test]
    fn build_validates() {
        let err = Permissions::builder()
            .add_read_permission(Permission::Glob("Vehicle.Speed".into()))
            .add_provide_permission(Permission::Glob("Vehicle Speed".into()))
            .build()
            .unwrap_err();
        assert_eq!(err, BuildError::InvalidGlob("Vehicle Speed".into()));

        let err = Permissions::builder()
            .add_read_permission(Permission::Glob(String::new()))
            .build()
            .unwrap_err();
        assert_eq!(err, BuildError::InvalidGlob(String::new()));
    }
}
