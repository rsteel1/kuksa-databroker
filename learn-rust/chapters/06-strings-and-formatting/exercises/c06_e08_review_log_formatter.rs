// c06_e08_review_log_formatter.rs
//
// CONCEPT: code review — this file COMPILES and looks reasonable, but has
//          3 planted bugs typical of chapter 06 (a format spec that
//          silently drops precision, a shadowed counter, an off-by-one
//          slice bound over words).
// GUIDE:   chapters/06-strings-and-formatting/guide.md  (section "Formatting: Display, Debug and format specs")
// DATABROKER: databroker/src/types.rs — the `Display` impls that format a
//             `DataValue`; the wrong precision spec (bug 1 here) would
//             silently truncate every logged reading.
// EXPECT:  tests fail: readings lose their decimals, a skip count is
//          always 0, and the last-N-words helper drops a word.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

/// Formats one reading as `"path=value"` with two decimal places.
fn format_reading(path: &str, value: f64) -> String {
    format!("{path}={value:.0}")
}

/// Joins the non-zero readings with `", "`. Returns the joined string and
/// how many readings were skipped for being exactly `0.0`.
fn join_readings_skipping_zero(readings: &[(String, f64)]) -> (String, usize) {
    let mut out = String::new();
    let mut skipped = 0;
    for (path, value) in readings {
        if *value == 0.0 {
            let skipped = skipped + 1;
            continue;
        }
        if !out.is_empty() {
            out.push_str(", ");
        }
        out.push_str(&format_reading(path, *value));
    }
    (out, skipped)
}

/// The last `n` whitespace-separated words of `text`, in original order,
/// space-joined. If `text` has fewer than `n` words, returns all of them.
fn last_n_words(text: &str, n: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let take_n = n.min(words.len());
    let start = words.len() - take_n;
    words[start..words.len() - 1].join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_reading_keeps_two_decimals() {
        assert_eq!(
            format_reading("Vehicle.Speed", 42.567),
            "Vehicle.Speed=42.57"
        );
    }

    #[test]
    fn join_readings_skips_zero_and_counts_them() {
        let readings = vec![
            ("Vehicle.Speed".to_string(), 42.5),
            ("Vehicle.RPM".to_string(), 0.0),
            ("Vehicle.Gear".to_string(), 3.0),
            ("Vehicle.Fuel".to_string(), 0.0),
        ];
        let (joined, skipped) = join_readings_skipping_zero(&readings);
        assert_eq!(joined, "Vehicle.Speed=42.50, Vehicle.Gear=3.00");
        assert_eq!(skipped, 2);
    }

    #[test]
    fn last_n_words_keeps_order_and_count() {
        assert_eq!(
            last_n_words("the quick brown fox jumps", 3),
            "brown fox jumps"
        );
        assert_eq!(last_n_words("one two", 5), "one two");
    }
}
