// c26_e08_review_invariant_guards.rs
//
// CONCEPT: code review — this file COMPILES, but has 2 planted bugs typical
//          of chapter 26 (an unsafe `as_str` whose soundness depends on an
//          invariant that two other, all-safe-looking methods quietly fail
//          to uphold).
// GUIDE:   chapters/26-unsafe-and-ffi/guide.md  (section "Invariants: why
//          privacy is a safety feature")
// DATABROKER: databroker/src/glob.rs — `Matcher::new` validates a pattern
//             once, at construction, so every later safe method can trust
//             it; databroker/src/vss.rs builds dotted signal paths the same
//             way. Neither ever needs `unsafe` — this file's `as_str` only
//             needs it because it skips the re-validation those designs do
//             for free, which is exactly why its two helpers below must
//             never let invalid data in.
// EXPECT:  tests fail: a constructor that should reject invalid UTF-8
//          silently accepts it, and a `truncate` that should refuse to
//          split a multi-byte character silently succeeds.
//
// `SignalPath` stores a signal path as raw bytes but promises those bytes
// are ALWAYS valid UTF-8 — that promise is what makes the `unsafe` block in
// `as_str` sound. Neither test below ever calls `as_str` on a `SignalPath`
// these bugs have already corrupted: the point of a review is to catch an
// invariant violation at its source (a safe method that should have
// refused, but didn't), not to watch it misbehave later.
//
// THIS FILE HAS EXACTLY 2 BUGS.
//
// YOUR TASK
//   Find and fix both bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

#![allow(unsafe_code)]

use std::str::Utf8Error;

#[derive(Debug, PartialEq, Eq)]
pub struct NotCharBoundary(pub usize);

/// A signal path stored as bytes.
///
/// INVARIANT: `bytes` is always valid UTF-8. Every safe method must keep
/// this true, because `as_str` trusts it unconditionally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalPath {
    bytes: Vec<u8>,
}

impl SignalPath {
    /// Validates `bytes` once, up front.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Utf8Error> {
        let _ = std::str::from_utf8(&bytes);
        Ok(SignalPath { bytes })
    }

    pub fn as_str(&self) -> &str {
        // SAFETY: every constructor and mutator validates `bytes` as UTF-8
        // before this type is built or changed (see the INVARIANT above).
        unsafe { std::str::from_utf8_unchecked(&self.bytes) }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Shortens the path to `new_len` bytes. Fails if that would land in
    /// the middle of a multi-byte character.
    pub fn truncate(&mut self, new_len: usize) -> Result<(), NotCharBoundary> {
        if new_len >= self.bytes.len() {
            return Ok(());
        }
        self.bytes.truncate(new_len);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_bytes_are_rejected() {
        assert!(
            SignalPath::from_bytes(vec![b'V', 0xFF, b'x']).is_err(),
            "0xFF is never valid UTF-8 on its own"
        );
    }

    #[test]
    fn valid_bytes_round_trip() {
        let p = SignalPath::from_bytes(b"Vehicle.Speed".to_vec()).expect("valid");
        assert_eq!(p.as_str(), "Vehicle.Speed");
    }

    #[test]
    fn truncate_rejects_mid_character_cuts() {
        // "Tür": 'T' = 1 byte, 'ü' = bytes 1..3, 'r' = byte 3. Cutting at 2
        // would land inside 'ü'.
        let mut p = SignalPath::from_bytes("Tür".as_bytes().to_vec()).expect("valid");
        assert_eq!(p.truncate(2), Err(NotCharBoundary(2)));
        assert_eq!(p.len(), 4, "a rejected truncate must not change anything");
    }

    #[test]
    fn truncate_accepts_char_boundaries() {
        let mut p = SignalPath::from_bytes("Tür".as_bytes().to_vec()).expect("valid");
        assert_eq!(p.truncate(3), Ok(()));
        assert_eq!(p.as_str(), "Tü");
        assert_eq!(p.truncate(0), Ok(()));
        assert_eq!(p.as_str(), "");
    }
}
