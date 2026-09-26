// c33_e04_editions_and_msrv.rs
//
// CONCEPT: editions (2015/2018/2021/2024) change how the *same* source text
//          is read. The MSRV (minimum supported Rust version) is the oldest
//          compiler that can build a crate, i.e. the newest feature it uses.
// GUIDE:   chapters/33-cargo-ecosystem-api-design/guide.md  (section "Editions and MSRV")
// DATABROKER: databroker/Cargo.toml — `edition = "2021"`; the root
//             Cargo.toml sets `resolver = "2"` explicitly (a virtual workspace
//             doesn't infer it from an edition). No `rust-version` is declared.
// EXPECT:  compile error E0782 ("expected a type, found a trait": a bare trait object),
//          then tests fail: `todo!()` panics
//
// When you open an old crate (or old answers online) you'll meet code
// written for an earlier edition. Knowing which rules changed when lets you
// read it correctly instead of "fixing" code that was fine in its own edition.
//
// YOUR TASK
//   1. Fix `provider_names` so it compiles under edition 2021.
//   2. Implement `owned_paths` using the 2021 meaning of `array.into_iter()`.
//   3. Answer the quiz: implement `introduced_in` and `stabilized_in`. Look
//      the answers up (edition guide, release notes); the tests only store
//      hashes, so they can't give the answers away. Each wrong answer is
//      reported by name.
//   4. Implement `msrv`.
//   Do not edit the tests.

// ---- 1. bare trait objects ------------------------------------------------

pub trait Provider {
    fn name(&self) -> &str;
}

pub fn provider_names(providers: &[Box<dyn Provider>]) -> Vec<&str> {
    providers.iter().map(|p| p.name()).collect()
}

// ---- 2. arrays by value ------------------------------------------------------

/// Moves the Strings out of the array (no clones).
pub fn owned_paths(paths: [String; 3]) -> Vec<String> {
    // WHY: since 2021, `array.into_iter()` yields `T` by value. In 2015/2018
    // it resolved to `(&array).into_iter()` and yielded `&T`.
    paths.into_iter().collect()
}

// ---- 3. quiz ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Edition {
    E2015,
    E2018,
    E2021,
    E2024,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditionChange {
    /// `async` and `await` became keywords.
    AsyncAwaitKeywords,
    /// `extern crate foo;` no longer needed; paths start with `crate::`.
    UniformPathsNoExternCrate,
    /// `Box<Trait>` without `dyn` became a hard error.
    BareTraitObjectsAreHardErrors,
    /// Closures capture individual fields (`self.a`) instead of all of `self`.
    DisjointClosureCaptures,
    /// `array.into_iter()` yields items by value.
    ArrayIntoIteratorByValue,
    /// `TryFrom`, `TryInto` and `FromIterator` are in the prelude.
    TryFromInPrelude,
    /// Cargo's feature resolver "2" is the default.
    FeatureResolverV2Default,
    /// `-> impl Trait` captures all in-scope generic lifetimes by default.
    RpitCapturesAllLifetimes,
    /// `gen` is a reserved keyword.
    GenKeywordReserved,
    /// `extern "C" { ... }` blocks must be written `unsafe extern "C"`.
    UnsafeExternBlocks,
    /// `if let Some(x) = a && x > 3 { ... }` (let chains).
    LetChains,
}

impl EditionChange {
    pub const ALL: [EditionChange; 11] = [
        EditionChange::AsyncAwaitKeywords,
        EditionChange::UniformPathsNoExternCrate,
        EditionChange::BareTraitObjectsAreHardErrors,
        EditionChange::DisjointClosureCaptures,
        EditionChange::ArrayIntoIteratorByValue,
        EditionChange::TryFromInPrelude,
        EditionChange::FeatureResolverV2Default,
        EditionChange::RpitCapturesAllLifetimes,
        EditionChange::GenKeywordReserved,
        EditionChange::UnsafeExternBlocks,
        EditionChange::LetChains,
    ];
}

/// Which edition introduced the change?
pub fn introduced_in(change: EditionChange) -> Edition {
    use EditionChange::*;
    match change {
        AsyncAwaitKeywords | UniformPathsNoExternCrate => Edition::E2018,
        BareTraitObjectsAreHardErrors
        | DisjointClosureCaptures
        | ArrayIntoIteratorByValue
        | TryFromInPrelude
        | FeatureResolverV2Default => Edition::E2021,
        RpitCapturesAllLifetimes | GenKeywordReserved | UnsafeExternBlocks | LetChains => {
            Edition::E2024
        }
    }
}

/// A stable Rust release, e.g. `RustVersion(1, 70)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RustVersion(pub u32, pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdFeature {
    /// `let Some(x) = opt else { return };`
    LetElse,
    /// `std::thread::scope`
    ThreadScope,
    /// `std::sync::OnceLock`
    OnceLock,
    /// `Option::is_some_and`
    OptionIsSomeAnd,
    /// `async fn` directly in traits (no `#[async_trait]`)
    AsyncFnInTrait,
    /// `const { ... }` blocks in expressions
    InlineConst,
    /// `std::sync::LazyLock`
    LazyLock,
    /// `impl Trait + use<'a, T>` precise capturing
    PreciseCapturingUse,
    /// Edition 2024 itself
    Edition2024,
}

impl StdFeature {
    pub const ALL: [StdFeature; 9] = [
        StdFeature::LetElse,
        StdFeature::ThreadScope,
        StdFeature::OnceLock,
        StdFeature::OptionIsSomeAnd,
        StdFeature::AsyncFnInTrait,
        StdFeature::InlineConst,
        StdFeature::LazyLock,
        StdFeature::PreciseCapturingUse,
        StdFeature::Edition2024,
    ];
}

/// The first stable release that has the feature.
pub fn stabilized_in(feature: StdFeature) -> RustVersion {
    match feature {
        StdFeature::ThreadScope => RustVersion(1, 63),
        StdFeature::LetElse => RustVersion(1, 65),
        StdFeature::OnceLock | StdFeature::OptionIsSomeAnd => RustVersion(1, 70),
        StdFeature::AsyncFnInTrait => RustVersion(1, 75),
        StdFeature::InlineConst => RustVersion(1, 79),
        StdFeature::LazyLock => RustVersion(1, 80),
        StdFeature::PreciseCapturingUse => RustVersion(1, 82),
        StdFeature::Edition2024 => RustVersion(1, 85),
    }
}

// ---- 4. MSRV ------------------------------------------------------------------

/// The minimum Rust version for code using `features`: the newest one.
/// No features → `RustVersion(1, 0)`.
pub fn msrv(features: &[StdFeature]) -> RustVersion {
    features
        .iter()
        .map(|&f| stabilized_in(f))
        .max()
        .unwrap_or(RustVersion(1, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Named(&'static str);
    impl Provider for Named {
        fn name(&self) -> &str {
            self.0
        }
    }

    /// FNV-1a: a tiny, stable hash so the answers aren't stored in plain text.
    fn fnv1a(s: &str) -> u64 {
        s.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
    }

    #[test]
    fn dyn_trait_objects() {
        let providers: Vec<Box<dyn Provider>> =
            vec![Box::new(Named("can")), Box::new(Named("gps"))];
        assert_eq!(provider_names(&providers), ["can", "gps"]);
    }

    #[test]
    fn arrays_iterate_by_value() {
        let paths = [
            "Vehicle.Speed".to_string(),
            "Vehicle.Cabin.Door".to_string(),
            "Vehicle.VIN".to_string(),
        ];
        let owned: Vec<String> = owned_paths(paths);
        assert_eq!(owned[2], "Vehicle.VIN");
    }

    const EDITION_ANSWERS: [(EditionChange, u64); 11] = [
        (EditionChange::AsyncAwaitKeywords, 0x5bcf_23b1_2f23_1ece),
        (
            EditionChange::UniformPathsNoExternCrate,
            0xe9b1_1afb_b9d0_77aa,
        ),
        (
            EditionChange::BareTraitObjectsAreHardErrors,
            0x8aac_09f8_0b72_c90e,
        ),
        (
            EditionChange::DisjointClosureCaptures,
            0x8f87_7189_1cd7_3c7c,
        ),
        (
            EditionChange::ArrayIntoIteratorByValue,
            0x8fa6_6d3a_cf53_3731,
        ),
        (EditionChange::TryFromInPrelude, 0xb128_cfd9_58f5_f087),
        (
            EditionChange::FeatureResolverV2Default,
            0x95af_780d_c113_f8a1,
        ),
        (
            EditionChange::RpitCapturesAllLifetimes,
            0x0f39_fbe4_619d_c802,
        ),
        (EditionChange::GenKeywordReserved, 0x0ec5_54ce_300d_afc2),
        (EditionChange::UnsafeExternBlocks, 0xcecf_7654_1401_381d),
        (EditionChange::LetChains, 0x8f7c_b3cd_f0c9_a0d6),
    ];

    const RELEASE_ANSWERS: [(StdFeature, u64); 9] = [
        (StdFeature::LetElse, 0xb284_7a90_5e10_48d1),
        (StdFeature::ThreadScope, 0x06c9_1c23_a310_d4db),
        (StdFeature::OnceLock, 0x128d_a594_f2a2_1c87),
        (StdFeature::OptionIsSomeAnd, 0x4758_9513_bb30_4535),
        (StdFeature::AsyncFnInTrait, 0x9fda_8e6a_73dd_384f),
        (StdFeature::InlineConst, 0x8c30_f903_0f8c_f41e),
        (StdFeature::LazyLock, 0xa5f1_a451_7598_1dff),
        (StdFeature::PreciseCapturingUse, 0x7d28_7a92_a81e_3eef),
        (StdFeature::Edition2024, 0xe724_7ee6_8b5a_1a41),
    ];

    #[test]
    fn edition_quiz() {
        let wrong: Vec<EditionChange> = EDITION_ANSWERS
            .iter()
            .filter(|(change, hash)| {
                fnv1a(&format!("{change:?}={:?}", introduced_in(*change))) != *hash
            })
            .map(|(change, _)| *change)
            .collect();
        assert!(wrong.is_empty(), "wrong edition for: {wrong:?}");
        assert_eq!(EditionChange::ALL.len(), EDITION_ANSWERS.len());
    }

    #[test]
    fn release_quiz() {
        let wrong: Vec<StdFeature> = RELEASE_ANSWERS
            .iter()
            .filter(|(feature, hash)| {
                fnv1a(&format!("{feature:?}={:?}", stabilized_in(*feature))) != *hash
            })
            .map(|(feature, _)| *feature)
            .collect();
        assert!(wrong.is_empty(), "wrong release for: {wrong:?}");
        assert_eq!(StdFeature::ALL.len(), RELEASE_ANSWERS.len());
    }

    #[test]
    fn msrv_is_the_newest_feature_used() {
        assert_eq!(msrv(&[]), RustVersion(1, 0));
        for feature in StdFeature::ALL {
            assert_eq!(msrv(&[feature]), stabilized_in(feature));
        }
        let all = msrv(&StdFeature::ALL);
        assert!(StdFeature::ALL.iter().all(|&f| stabilized_in(f) <= all));
        assert!(StdFeature::ALL.iter().any(|&f| stabilized_in(f) == all));
    }
}
