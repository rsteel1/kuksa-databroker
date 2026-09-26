// c32_e05_property_testing.rs
//
// CONCEPT: property-based testing with `proptest`: describe the *inputs*
//          (a strategy) and a *property* that must hold for all of them;
//          the library generates hundreds of cases and shrinks failures to
//          a minimal example.
// GUIDE:   chapters/32-testing/guide.md  (section "Property-based testing with proptest")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/conversions.rs — `From<proto::Value>
//             for DataValue` and back: conversions that must round-trip for
//             every value, which is exactly what a property states.
// EXPECT:  tests fail: `todo!()` panics in `arb_value`. Once you implement
//          it, the round-trip property fails with a shrunk counterexample.
//
// `encode`/`decode` below turn a signal value into one line of text and
// back. They pass the hand-written example tests at the bottom, and they
// have two bugs that none of those examples hit. You'll find them the way
// property testing finds bugs in real code: let the machine choose inputs.
//
// YOUR TASK
//   1. Implement `arb_value()`, a `Strategy` that generates every `Value`
//      variant, including the awkward ones: strings containing `\` and
//      newlines, negative numbers, empty arrays. The test
//      `strategy_covers_the_tricky_cases` checks your generator.
//   2. Run the tests. The round-trip property fails; read the minimal
//      counterexample proptest prints, then fix `encode`/`decode`.
//   Keep the text format the example tests pin down. Do not edit the tests.

use proptest::prelude::*;
use proptest::test_runner::{RngSeed, TestRunner};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Str(String),
    IntArray(Vec<i64>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    UnknownTag(String),
    BadBool(String),
    BadInt(String),
    BadEscape,
}

/// Encodes a value as one line: `bool:true`, `int:-5`, `str:<escaped>`,
/// `ints:1,2,3`. In strings, `\` is written as `\\` and a newline as `\n`,
/// so the output never contains a real newline.
pub fn encode(value: &Value) -> String {
    match value {
        Value::Bool(b) => format!("bool:{b}"),
        Value::Int(i) => format!("int:{i}"),
        Value::Str(s) => {
            let mut out = String::with_capacity(s.len() + 4);
            out.push_str("str:");
            for c in s.chars() {
                match c {
                    // WHY (bug 1): the backslash itself must be escaped too,
                    // or a literal `\n` in the input decodes as a newline.
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    other => out.push(other),
                }
            }
            out
        }
        Value::IntArray(values) => {
            let items: Vec<String> = values.iter().map(i64::to_string).collect();
            format!("ints:{}", items.join(","))
        }
    }
}

pub fn decode(line: &str) -> Result<Value, DecodeError> {
    let (tag, rest) = line
        .split_once(':')
        .ok_or_else(|| DecodeError::UnknownTag(line.to_string()))?;
    match tag {
        "bool" => match rest {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            other => Err(DecodeError::BadBool(other.to_string())),
        },
        "int" => rest
            .parse()
            .map(Value::Int)
            .map_err(|_| DecodeError::BadInt(rest.to_string())),
        "str" => {
            let mut out = String::with_capacity(rest.len());
            let mut chars = rest.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    match chars.next() {
                        Some('\\') => out.push('\\'),
                        Some('n') => out.push('\n'),
                        _ => return Err(DecodeError::BadEscape),
                    }
                } else {
                    out.push(c);
                }
            }
            Ok(Value::Str(out))
        }
        "ints" => {
            // WHY (bug 2): "".split(',') yields one empty item, so the empty
            // array needs its own case.
            if rest.is_empty() {
                return Ok(Value::IntArray(Vec::new()));
            }
            rest.split(',')
                .map(|item| {
                    item.parse()
                        .map_err(|_| DecodeError::BadInt(item.to_string()))
                })
                .collect::<Result<Vec<i64>, _>>()
                .map(Value::IntArray)
        }
        other => Err(DecodeError::UnknownTag(other.to_string())),
    }
}

/// Generates arbitrary `Value`s, biased towards the cases that break codecs.
pub fn arb_value() -> impl Strategy<Value = Value> {
    // WHY: a regex strategy mixing plain letters with the two escape-worthy
    // characters makes backslashes and newlines common instead of rare.
    let tricky_string = "[a-z\\\\\n,:]{0,12}";
    prop_oneof![
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::Int),
        tricky_string.prop_map(Value::Str),
        any::<String>().prop_map(Value::Str),
        prop::collection::vec(any::<i64>(), 0..5).prop_map(Value::IntArray),
    ]
}

/// Fixed seed and no failure files: every run tests the same cases, and
/// nothing is written next to the source.
fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 512,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(0x5EED_CAFE),
        ..ProptestConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::strategy::ValueTree;

    // ---- example-based tests: these pass even with the bugs ----

    #[test]
    fn examples_encode() {
        assert_eq!(encode(&Value::Bool(true)), "bool:true");
        assert_eq!(encode(&Value::Int(-5)), "int:-5");
        assert_eq!(encode(&Value::Str("a\nb".into())), "str:a\\nb");
        assert_eq!(encode(&Value::IntArray(vec![1, -2, 3])), "ints:1,-2,3");
    }

    #[test]
    fn examples_decode() {
        assert_eq!(decode("int:42"), Ok(Value::Int(42)));
        assert_eq!(decode("str:hello"), Ok(Value::Str("hello".into())));
        assert_eq!(decode("ints:7"), Ok(Value::IntArray(vec![7])));
        assert_eq!(
            decode("bool:maybe"),
            Err(DecodeError::BadBool("maybe".into()))
        );
        assert_eq!(
            decode("float:1.0"),
            Err(DecodeError::UnknownTag("float".into()))
        );
        assert_eq!(decode("str:bad\\q"), Err(DecodeError::BadEscape));
    }

    // ---- the generator must reach the corners ----

    #[test]
    fn strategy_covers_the_tricky_cases() {
        let mut runner = TestRunner::deterministic();
        let strategy = arb_value();
        let (mut bools, mut negatives, mut empty_arrays) = (0, 0, 0);
        let (mut backslashes, mut newlines) = (0, 0);
        for _ in 0..2000 {
            let value = strategy
                .new_tree(&mut runner)
                .expect("strategy generates values")
                .current();
            match value {
                Value::Bool(_) => bools += 1,
                Value::Int(i) if i < 0 => negatives += 1,
                Value::Int(_) => {}
                Value::Str(s) => {
                    backslashes += usize::from(s.contains('\\'));
                    newlines += usize::from(s.contains('\n'));
                }
                Value::IntArray(v) => empty_arrays += usize::from(v.is_empty()),
            }
        }
        assert!(bools > 0, "never generated a Bool");
        assert!(negatives > 0, "never generated a negative Int");
        assert!(empty_arrays > 0, "never generated an empty IntArray");
        assert!(
            backslashes > 20,
            "strings rarely contain a backslash ({backslashes}/2000)"
        );
        assert!(
            newlines > 20,
            "strings rarely contain a newline ({newlines}/2000)"
        );
    }

    // ---- the properties ----

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn round_trips(value in arb_value()) {
            let line = encode(&value);
            prop_assert_eq!(decode(&line), Ok(value), "encoded as {:?}", line);
        }

        #[test]
        fn encoding_is_a_single_line(value in arb_value()) {
            prop_assert!(!encode(&value).contains('\n'));
        }
    }
}
