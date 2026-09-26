// c06_e01_utf8_bytes_chars.rs
//
// CONCEPT: a `str` is UTF-8 bytes; `len()` counts bytes, `chars()` walks characters.
// GUIDE:   chapters/06-strings-and-formatting/guide.md  (section "UTF-8: bytes, chars and why s[0] doesn't compile")
// DATABROKER: databroker/src/glob.rs — `is_valid_path` and `Matcher` work on
//             `&str` with methods (`replace`, `ends_with`), never with byte indexes.
// EXPECT:  compile error E0277 (the type `str` cannot be indexed by `{integer}`),
//          then failing tests.
//
// Signal paths are usually ASCII ("Vehicle.Speed"), but descriptions, units
// and custom VSS overlays may contain "°C", "Größe" or emoji. Code that
// assumes one byte == one character works on every test you wrote with
// ASCII and panics in the car.
//
// YOUR TASK
//   Fix the three functions so they are correct for any UTF-8 text:
//   - `first_char` must not index the string with a number;
//   - `char_len` must count characters, not bytes;
//   - `truncate_chars` must never panic, even in the middle of a multi-byte char.
//   Do not edit the tests.

// I AM NOT DONE

/// The first character of `s`, if any.
fn first_char(s: &str) -> Option<char> {
    if s.is_empty() {
        None
    } else {
        Some(s[0])
    }
}

/// Number of characters (Unicode scalar values), not bytes.
fn char_len(s: &str) -> usize {
    s.len()
}

/// The longest prefix of `s` that has at most `max` characters.
fn truncate_chars(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_char_ascii_and_unicode() {
        assert_eq!(first_char("Vehicle"), Some('V'));
        assert_eq!(first_char("Ölstand"), Some('Ö'));
        assert_eq!(first_char(""), None);
    }

    #[test]
    fn len_is_bytes_chars_is_chars() {
        // "Größe": G r ö ß e -> 5 chars, but ö and ß take 2 bytes each.
        assert_eq!("Größe".len(), 7);
        assert_eq!(char_len("Größe"), 5);
        assert_eq!(char_len("Vehicle.Speed"), 13);
        assert_eq!(char_len("🚗"), 1);
    }

    #[test]
    fn truncate_never_splits_a_char() {
        assert_eq!(truncate_chars("Größe", 3), "Grö");
        assert_eq!(truncate_chars("Speed", 10), "Speed");
        assert_eq!(truncate_chars("🚗car", 1), "🚗");
        assert_eq!(truncate_chars("abc", 0), "");
    }
}
