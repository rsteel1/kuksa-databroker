// c10_e08_review_sample_pipeline.rs
//
// CONCEPT: code review. The file below COMPILES and its three functions all
//          return a value of the right type, but each has one planted bug
//          typical of iterator/closure code: an adaptor that's never
//          consumed, an error silently discarded instead of propagated, and
//          a comparator that sorts the wrong way.
// GUIDE:   chapters/10-closures-and-iterators/guide.md  (section "Iterator adaptors and collect")
// DATABROKER: databroker/src/vss.rs uses `collect::<Result<_, _>>()` so one
//          bad entry fails the whole parse instead of silently vanishing —
//          exactly the property `parse_all` below is supposed to have.
// EXPECT:  compiles; 3 of the tests below fail.
//
// THIS FILE HAS 3 BUGS. All three compile cleanly — the mistakes are
// logical, not type errors — but they make specific tests fail.
//
// YOUR TASK
//   Find and fix all 3 bugs. Do not change the tests or the signatures. For
//   each bug, add a one-line comment directly above your fix:
//   `// BUG 1: <what was wrong>`, `// BUG 2: ...`, `// BUG 3: ...`.

// I AM NOT DONE

#[derive(Debug, Clone, PartialEq)]
struct Sample {
    path: String,
    value: f64,
}

/// Appends the path of every sample below `threshold` to `tagged`, in order.
fn tag_low_battery(samples: &[Sample], threshold: f64, tagged: &mut Vec<String>) {
    samples
        .iter()
        .filter(|s| s.value < threshold)
        .map(|s| tagged.push(s.path.clone()));
}

/// Parses every input as an `f64`. If any input fails to parse, returns the
/// error message for the *first* one that fails — it must not silently drop
/// bad entries and return the good ones.
fn parse_all(inputs: &[&str]) -> Result<Vec<f64>, String> {
    Ok(inputs
        .iter()
        .filter_map(|s| s.parse::<f64>().ok())
        .collect())
}

/// The paths of `samples`, ordered by `value` from highest to lowest.
fn sorted_paths_by_value_desc(samples: &[Sample]) -> Vec<String> {
    let mut sorted: Vec<&Sample> = samples.iter().collect();
    sorted.sort_by(|a, b| a.value.partial_cmp(&b.value).unwrap());
    sorted.into_iter().map(|s| s.path.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(path: &str, value: f64) -> Sample {
        Sample {
            path: path.to_string(),
            value,
        }
    }

    #[test]
    fn tag_low_battery_actually_tags() {
        let samples = vec![s("a", 1.0), s("b", 9.0), s("c", 2.0)];
        let mut tagged = Vec::new();
        tag_low_battery(&samples, 5.0, &mut tagged);
        assert_eq!(tagged, vec!["a".to_string(), "c".to_string()]);
    }

    #[test]
    fn parse_all_succeeds_when_everything_is_valid() {
        assert_eq!(parse_all(&["1.0", "2.5", "3.0"]), Ok(vec![1.0, 2.5, 3.0]));
    }

    #[test]
    fn parse_all_reports_the_first_bad_entry_instead_of_dropping_it() {
        let result = parse_all(&["1.0", "bad", "3.0"]);
        assert_eq!(result, Err("bad value: bad".to_string()));
    }

    #[test]
    fn sorted_paths_by_value_desc_is_actually_descending() {
        let samples = vec![s("low", 1.0), s("high", 9.0), s("mid", 5.0)];
        assert_eq!(
            sorted_paths_by_value_desc(&samples),
            vec!["high".to_string(), "mid".to_string(), "low".to_string()]
        );
    }
}
