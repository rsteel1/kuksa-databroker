// c08_e06_review_module_boundary.rs
//
// CONCEPT: code review — this file COMPILES and looks reasonable, but has
//          3 planted bugs typical of chapter 08 (an off-by-one segment
//          count, a `Result` discarded right at a module boundary, a
//          shadowed accumulator).
// GUIDE:   chapters/08-modules-and-cargo/guide.md  (section "Modules and privacy")
// DATABROKER: databroker/src/glob.rs — path validation lives in its own
//             module and returns a `Result`; a caller across the module
//             boundary that discards it with `let _ = ...` (bug 2 here)
//             would silently store invalid paths.
// EXPECT:  tests fail: `segment_count` is off by one, an invalid path
//          isn't skipped, and the rolling average is always zero.
//
// THIS FILE HAS EXACTLY 3 BUGS.
//
// YOUR TASK
//   Find and fix all 3 bugs. For each one, add a one-line comment right
//   above the fix: `// BUG 1: <what was wrong>`, `// BUG 2: ...`,
//   `// BUG 3: ...`.
//   Do not edit the tests, and do not change any function's signature.

// I AM NOT DONE

mod pathing {
    /// `Ok(())` if `path` is a syntactically valid VSS path (non-empty,
    /// dot-separated segments of ASCII alphanumerics/underscore).
    pub fn validate(path: &str) -> Result<(), String> {
        if path.is_empty() {
            return Err("empty path".to_string());
        }
        for segment in path.split('.') {
            if segment.is_empty()
                || !segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(format!("invalid segment {segment:?} in {path:?}"));
            }
        }
        Ok(())
    }

    /// How many dot-separated segments `path` has.
    pub fn segment_count(path: &str) -> usize {
        path.split('.').count() - 1
    }
}

mod stats {
    /// Registers `path` in `store`, but ONLY if it passes
    /// `pathing::validate` — an invalid path must be silently skipped,
    /// not stored.
    pub fn register_valid(store: &mut Vec<String>, path: &str) {
        let _ = super::pathing::validate(path);
        store.push(path.to_string());
    }

    /// The centered rolling average of `readings` over the last
    /// `window` values (fewer at the start).
    pub fn rolling_average(readings: &[f64], window: usize) -> Vec<f64> {
        let mut out = Vec::new();
        for i in 0..readings.len() {
            let start = i.saturating_sub(window - 1);
            let mut sum = 0.0;
            for value in &readings[start..=i] {
                let sum = sum + value;
            }
            out.push(sum / (i - start + 1) as f64);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_count_counts_every_segment() {
        assert_eq!(pathing::segment_count("Vehicle"), 1);
        assert_eq!(pathing::segment_count("Vehicle.Cabin.Door"), 3);
    }

    #[test]
    fn register_valid_skips_invalid_paths() {
        let mut store = Vec::new();
        stats::register_valid(&mut store, "Vehicle.Speed");
        stats::register_valid(&mut store, "Vehicle..Bad");
        assert_eq!(store, vec!["Vehicle.Speed".to_string()]);
    }

    #[test]
    fn rolling_average_is_correct() {
        let readings = [1.0, 2.0, 3.0, 4.0];
        let result = stats::rolling_average(&readings, 2);
        assert_eq!(result, vec![1.0, 1.5, 2.5, 3.5]);
    }
}
