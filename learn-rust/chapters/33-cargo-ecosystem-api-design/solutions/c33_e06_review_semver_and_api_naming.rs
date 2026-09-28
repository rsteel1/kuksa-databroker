// c33_e06_review_semver_and_api_naming.rs
//
// CONCEPT: code review — this file COMPILES, but has 3 planted bugs at the
//          intersection of SemVer classification and API correctness: a
//          change classified as non-breaking when it is breaking, an `as
//          i32` cast that quietly reinterprets a large `u32`'s bit pattern
//          instead of comparing it as the unsigned number it is, and a
//          formatter (named the idiomatic way — no `get_` prefix, per the
//          Rust API Guidelines' C-GETTER — but wrong all the same) that
//          silently drops part of its own output.
// GUIDE:   chapters/33-cargo-ecosystem-api-design/guide.md  (section
//          "SemVer: the contract behind every version number" and
//          "Designing APIs other people enjoy")
// DATABROKER: Cargo book, "SemVer Compatibility" — adding a public field to
//             a struct whose fields are ALL public is breaking (existing
//             `Struct { a, b }` literals stop compiling); the databroker's
//             own `Datapoint { ts, source_ts, value }` (broker.rs) is an
//             example of exactly that struct shape.
// EXPECT:  tests fail: adding a public field to an all-public-fields struct
//          is classified as a minor (non-breaking) release, a version
//          bumped past `i32::MAX` compares as a small negative number
//          instead of a large one, and the formatted version string drops
//          the patch number.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not rename any function, and do not edit the tests.

/// A change to a library's public surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    AddPublicItem,
    /// e.g. adding `pub unit: String` to `pub struct Datapoint { pub ts, pub value }`.
    AddPublicFieldToStructWithOnlyPublicFields,
    RemovePublicItem,
}

/// What kind of release a change needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Bump {
    Minor,
    Major,
}

/// Which release `change` requires, following the Cargo book's "SemVer
/// Compatibility" chapter.
fn required_bump(change: Change) -> Bump {
    match change {
        Change::AddPublicItem => Bump::Minor,
        // BUG 1: adding a public field to a struct whose fields are ALL
        // already public is breaking — every existing `Struct { a, b }`
        // literal (with no `..Default::default()`) stops compiling once a
        // new field appears. (Adding a field to a struct that already had
        // at least one private field is Minor: nobody outside the crate
        // could build one with a literal in the first place.)
        Change::AddPublicFieldToStructWithOnlyPublicFields => Bump::Major,
        Change::RemovePublicItem => Bump::Major,
    }
}

/// A `major.minor.patch` version, stored as one packed `u32` the way some
/// space-constrained wire formats do: `major * 1_000_000 + minor * 1_000 + patch`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PackedVersion(u32);

impl PackedVersion {
    fn new(major: u32, minor: u32, patch: u32) -> Self {
        PackedVersion(major * 1_000_000 + minor * 1_000 + patch)
    }

    /// Is `self` newer than `other`? Packed versions compare correctly as
    /// plain integers, as long as the comparison itself doesn't change sign.
    fn is_newer_than(&self, other: &PackedVersion) -> bool {
        // BUG 2: `self.0` is a `u32`; casting it `as i32` reinterprets its
        // bit pattern rather than converting its value, so any packed
        // version at or above `i32::MAX + 1` (about major version 2147)
        // comes out negative. There is no sign problem to work around:
        // compare the `u32`s directly.
        self.0 > other.0
    }

    fn major(&self) -> u32 {
        self.0 / 1_000_000
    }

    fn minor(&self) -> u32 {
        (self.0 / 1_000) % 1_000
    }

    fn patch(&self) -> u32 {
        self.0 % 1_000
    }

    /// Renders as `"MAJOR.MINOR.PATCH"`.
    fn version_string(&self) -> String {
        // BUG 3: the doc comment promises `MAJOR.MINOR.PATCH`, but the
        // format string only has two placeholders, silently dropping the
        // patch number from every version string.
        format!("{}.{}.{}", self.major(), self.minor(), self.patch())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_a_public_field_to_an_all_public_struct_is_breaking() {
        // Existing call sites doing `Datapoint { ts, value }` (no `..Default::default()`)
        // stop compiling once a new public field appears: this is Major, not Minor.
        assert_eq!(
            required_bump(Change::AddPublicFieldToStructWithOnlyPublicFields),
            Bump::Major
        );
        // Adding a whole new item, or one to a struct that already had a
        // private field (so nobody could build it with a literal), stays Minor.
        assert_eq!(required_bump(Change::AddPublicItem), Bump::Minor);
    }

    #[test]
    fn packed_versions_compare_correctly_past_a_million_major_versions() {
        let old = PackedVersion::new(1, 46, 1);
        let new = PackedVersion::new(3_000, 0, 0); // packs to > i32::MAX
        assert!(
            new.is_newer_than(&old),
            "3000.0.0 must compare newer than 1.46.1"
        );
    }

    #[test]
    fn version_string_includes_the_patch_number() {
        let v = PackedVersion::new(1, 46, 1);
        assert_eq!(v.version_string(), "1.46.1");
    }
}
