// c26_e03_safe_abstraction_invariants.rs
//
// CONCEPT: an unsafe block is only as sound as the *invariant* every safe
//          method maintains; private fields are what make that possible.
// GUIDE:   chapters/26-unsafe-and-ffi/guide.md  (section "Invariants: why privacy is a safety feature")
// DATABROKER: databroker/src/glob.rs — `Matcher` keeps `glob_pattern` private
//             and only builds it through `Matcher::new`, which validates it.
//             That's the same idea (validated state behind private fields)
//             without unsafe. Here the invariant carries memory safety.
// EXPECT:  tests fail: todo!() panics
//
// `PathBuf8` stores a signal path as raw bytes (`Vec<u8>`) but promises that
// those bytes are ALWAYS valid UTF-8. That promise lets `as_str` use
// `str::from_utf8_unchecked` (no re-validation on every read). The unsafe
// block in `as_str` is correct only if EVERY other method keeps the
// invariant, including methods that contain no `unsafe` at all.
//
// YOUR TASK
//   Implement the four `todo!()` methods.
//   - `from_bytes` validates (use `std::str::from_utf8`).
//   - `as_str` uses `from_utf8_unchecked` with a `// SAFETY:` comment that
//     names the invariant.
//   - `truncate` must refuse to cut in the middle of a multi-byte character.
//   - `pop_segment` removes the last `.segment` (think `Vehicle.Speed` ->
//     `Vehicle`), returning it as an owned `String`.
//   Ask yourself for each method: "could a safe caller use this to put
//   invalid UTF-8 into `bytes`?" If yes, `as_str` becomes Undefined Behaviour.
//   Do not edit the tests.

// I AM NOT DONE

#![allow(unsafe_code)]

use std::str::Utf8Error;

/// A signal path stored as bytes.
///
/// INVARIANT: `bytes` is always valid UTF-8.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PathBuf8 {
    // Private on purpose: a `pub` field would let any safe code break the
    // invariant, making `as_str` unsound.
    bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NotCharBoundary(pub usize);

impl PathBuf8 {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates `bytes` once, up front.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Utf8Error> {
        let _ = bytes;
        todo!()
    }

    /// Appending a `&str` can't break the invariant: `&str` is already UTF-8.
    pub fn push_str(&mut self, s: &str) {
        self.bytes.extend_from_slice(s.as_bytes());
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn as_str(&self) -> &str {
        todo!()
    }

    /// Shortens to `new_len` bytes. Fails if that would split a character.
    pub fn truncate(&mut self, new_len: usize) -> Result<(), NotCharBoundary> {
        let _ = new_len;
        todo!()
    }

    /// Removes and returns the last `.`-separated segment.
    /// `"Vehicle.Cabin.Temp"` -> returns `"Temp"`, leaves `"Vehicle.Cabin"`.
    /// A path without a dot is removed entirely. Empty path -> `None`.
    pub fn pop_segment(&mut self) -> Option<String> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_bytes_round_trip() {
        let p = PathBuf8::from_bytes(b"Vehicle.Speed".to_vec()).expect("valid");
        assert_eq!(p.as_str(), "Vehicle.Speed");
    }

    #[test]
    fn invalid_bytes_rejected() {
        assert!(PathBuf8::from_bytes(vec![b'V', 0xFF, b'x']).is_err());
    }

    #[test]
    fn push_and_read() {
        let mut p = PathBuf8::new();
        p.push_str("Vehicle.");
        p.push_str("Fahrzeugtür"); // 'ü' is 2 bytes
        assert_eq!(p.as_str(), "Vehicle.Fahrzeugtür");
        assert_eq!(p.len(), "Vehicle.Fahrzeugtür".len());
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        let mut p = PathBuf8::new();
        p.push_str("Tür");
        // "T" = 1 byte, "ü" = bytes 1..3, "r" = byte 3
        assert_eq!(p.truncate(2), Err(NotCharBoundary(2)));
        assert_eq!(
            p.as_str(),
            "Tür",
            "a failed truncate must not change anything"
        );
        assert_eq!(p.truncate(3), Ok(()));
        assert_eq!(p.as_str(), "Tü");
        assert_eq!(p.truncate(10), Ok(()));
        assert_eq!(p.as_str(), "Tü");
    }

    #[test]
    fn pop_segments() {
        let mut p = PathBuf8::new();
        p.push_str("Vehicle.Cabin.Tür");
        assert_eq!(p.pop_segment().as_deref(), Some("Tür"));
        assert_eq!(p.as_str(), "Vehicle.Cabin");
        assert_eq!(p.pop_segment().as_deref(), Some("Cabin"));
        assert_eq!(p.pop_segment().as_deref(), Some("Vehicle"));
        assert_eq!(p.as_str(), "");
        assert_eq!(p.pop_segment(), None);
    }
}
