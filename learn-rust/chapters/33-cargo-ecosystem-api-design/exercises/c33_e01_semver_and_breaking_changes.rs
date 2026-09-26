// c33_e01_semver_and_breaking_changes.rs
//
// CONCEPT: semantic versioning as Cargo applies it: which API changes are
//          breaking, how 0.x versions shift the rules, and what a caret
//          requirement like `tonic = "0.14"` actually accepts.
// GUIDE:   chapters/33-cargo-ecosystem-api-design/guide.md  (section "SemVer: the contract behind every version number")
// DATABROKER: Cargo.toml (repo root) — `[workspace.dependencies]`:
//             `tokio = "1.46.1"` accepts any 1.x ≥ 1.46.1, but `tonic = "0.14"`
//             only accepts 0.14.x, because for 0.x crates the *minor* number is
//             the breaking one.
// EXPECT:  tests fail: `todo!()` panics
//
// Every `version = "..."` in a Cargo.toml is a promise check: Cargo will
// silently pick the newest version that "matches". That only works because
// crate authors bump the right number. You'll be on both sides of this
// contract: depending on crates, and (as a contributor) changing a crate's API.
//
// YOUR TASK
//   Implement `Version::parse`, `Version::bump`, `required_bump`,
//   `next_version` and `caret_matches`. The doc comments are the spec.
//   Do not edit the tests.

// I AM NOT DONE

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseVersionError {
    /// The string did not have exactly three dot-separated parts.
    WrongNumberOfParts(usize),
    /// One part was not a non-negative integer.
    InvalidNumber(String),
}

impl fmt::Display for ParseVersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongNumberOfParts(n) => write!(f, "expected 3 version parts, found {n}"),
            Self::InvalidNumber(part) => write!(f, "invalid version number `{part}`"),
        }
    }
}

impl std::error::Error for ParseVersionError {}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What kind of release a change needs, in 1.x terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bump {
    Patch,
    Minor,
    Major,
}

/// A change to a library's public surface (or its Cargo.toml).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    BugFixNoApiChange,
    DocsOnly,
    AddPublicItem,
    RemovePublicItem,
    RenamePublicItem,
    ChangeFunctionSignature,
    /// e.g. adding `pub unit: String` to `pub struct Datapoint { pub ts, pub value }`
    AddPublicFieldToStructWithOnlyPublicFields,
    /// the struct already had a private field, so nobody could build it with a literal
    AddPrivateFieldToStructWithPrivateFields,
    AddVariantToExhaustiveEnum,
    AddVariantToNonExhaustiveEnum,
    AddTraitMethodWithDefault,
    AddTraitMethodWithoutDefault,
    AddCargoFeature,
    RemoveCargoFeature,
    RaiseMinimumRustVersion,
}

impl Version {
    /// Parses `"MAJOR.MINOR.PATCH"` (no pre-release/build metadata).
    pub fn parse(s: &str) -> Result<Version, ParseVersionError> {
        todo!()
    }

    /// Applies a bump.
    ///
    /// For `1.x` and later: Major → `(x+1).0.0`, Minor → `x.(y+1).0`,
    /// Patch → `x.y.(z+1)`.
    ///
    /// For `0.y.z` everything shifts one place right (that's Cargo's rule):
    /// Major → `0.(y+1).0`, Minor and Patch → `0.y.(z+1)`.
    pub fn bump(self, bump: Bump) -> Version {
        todo!()
    }

    pub fn new(major: u64, minor: u64, patch: u64) -> Version {
        Version {
            major,
            minor,
            patch,
        }
    }
}

/// The bump a single change requires, following the Cargo book's
/// "SemVer Compatibility" chapter. Treat "possibly-breaking" changes that the
/// book files under *minor* (new trait method with a default, raising the
/// MSRV) as `Minor`.
pub fn required_bump(change: Change) -> Bump {
    todo!()
}

/// The version to release after `changes`. The largest required bump wins.
/// No changes → the version is unchanged.
pub fn next_version(current: Version, changes: &[Change]) -> Version {
    todo!()
}

/// Does `candidate` satisfy the caret requirement `^req`?
/// (`version = "1.2.3"` in Cargo.toml means `^1.2.3`.)
///
/// - `^1.2.3` → `>=1.2.3, <2.0.0`
/// - `^0.2.3` → `>=0.2.3, <0.3.0`
/// - `^0.0.3` → exactly `0.0.3`
pub fn caret_matches(req: Version, candidate: Version) -> bool {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).expect("valid version in test")
    }

    #[test]
    fn parses_versions() {
        assert_eq!(v("1.46.1"), Version::new(1, 46, 1));
        assert_eq!(v("0.14.0"), Version::new(0, 14, 0));
        assert_eq!(
            Version::parse("1.2"),
            Err(ParseVersionError::WrongNumberOfParts(2))
        );
        assert_eq!(
            Version::parse("1.x.3"),
            Err(ParseVersionError::InvalidNumber("x".to_string()))
        );
        assert_eq!(v("0.7.2").to_string(), "0.7.2");
    }

    #[test]
    fn bumps_after_1_0() {
        assert_eq!(v("1.46.1").bump(Bump::Patch), v("1.46.2"));
        assert_eq!(v("1.46.1").bump(Bump::Minor), v("1.47.0"));
        assert_eq!(v("1.46.1").bump(Bump::Major), v("2.0.0"));
    }

    #[test]
    fn bumps_before_1_0_shift_right() {
        assert_eq!(v("0.14.6").bump(Bump::Major), v("0.15.0"));
        assert_eq!(v("0.14.6").bump(Bump::Minor), v("0.14.7"));
        assert_eq!(v("0.14.6").bump(Bump::Patch), v("0.14.7"));
    }

    #[test]
    fn classifies_changes() {
        use Change::*;
        let expected = [
            (BugFixNoApiChange, Bump::Patch),
            (DocsOnly, Bump::Patch),
            (AddPublicItem, Bump::Minor),
            (RemovePublicItem, Bump::Major),
            (RenamePublicItem, Bump::Major),
            (ChangeFunctionSignature, Bump::Major),
            (AddPublicFieldToStructWithOnlyPublicFields, Bump::Major),
            (AddPrivateFieldToStructWithPrivateFields, Bump::Minor),
            (AddVariantToExhaustiveEnum, Bump::Major),
            (AddVariantToNonExhaustiveEnum, Bump::Minor),
            (AddTraitMethodWithDefault, Bump::Minor),
            (AddTraitMethodWithoutDefault, Bump::Major),
            (AddCargoFeature, Bump::Minor),
            (RemoveCargoFeature, Bump::Major),
            (RaiseMinimumRustVersion, Bump::Minor),
        ];
        for (change, bump) in expected {
            assert_eq!(required_bump(change), bump, "{change:?}");
        }
    }

    #[test]
    fn next_version_takes_the_largest_bump() {
        use Change::*;
        assert_eq!(next_version(v("1.4.2"), &[]), v("1.4.2"));
        assert_eq!(next_version(v("1.4.2"), &[DocsOnly]), v("1.4.3"));
        assert_eq!(
            next_version(v("1.4.2"), &[DocsOnly, AddPublicItem]),
            v("1.5.0")
        );
        assert_eq!(
            next_version(v("1.4.2"), &[AddPublicItem, AddVariantToExhaustiveEnum]),
            v("2.0.0")
        );
        // The databroker is 0.7.x: a breaking change makes it 0.8.0.
        assert_eq!(
            next_version(v("0.7.2"), &[ChangeFunctionSignature]),
            v("0.8.0")
        );
        assert_eq!(next_version(v("0.7.2"), &[AddPublicItem]), v("0.7.3"));
    }

    #[test]
    fn caret_requirements() {
        // tokio = "1.46.1"
        assert!(caret_matches(v("1.46.1"), v("1.53.1")));
        assert!(!caret_matches(v("1.46.1"), v("1.46.0")));
        assert!(!caret_matches(v("1.46.1"), v("2.0.0")));
        // tonic = "0.14" (i.e. ^0.14.0)
        assert!(caret_matches(v("0.14.0"), v("0.14.6")));
        assert!(!caret_matches(v("0.14.0"), v("0.15.0")));
        assert!(!caret_matches(v("0.14.0"), v("1.0.0")));
        // ^0.0.3 is exact
        assert!(caret_matches(v("0.0.3"), v("0.0.3")));
        assert!(!caret_matches(v("0.0.3"), v("0.0.4")));
    }
}
