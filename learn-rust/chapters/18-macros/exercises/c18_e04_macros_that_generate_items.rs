// c18_e04_macros_that_generate_items.rs
//
// CONCEPT: macros that expand to *items* (enums, impl blocks, functions,
//          even `#[test]`s), killing copy-paste across many types.
// GUIDE:   chapters/18-macros/guide.md  (section "Generating items")
// DATABROKER: databroker/src/types.rs — `enum DataType` (line 17) and its
//             hand-written `Display` (line 44) list all 24 variants twice;
//             databroker-proto/src/lib.rs has a third hand-written list in
//             `impl FromStr for DataType` (line 49). A macro can keep one list.
// EXPECT:  compile errors E0425/E0433 (cannot find type `DataType`) and
//          E0277 (`SignalValue` is not implemented): both macros expand to
//          nothing yet.
//
// A macro invoked at module level can expand to any number of items. A
// single table of `variant => "name"` pairs can generate the enum, its
// `Display`, its `FromStr` and an `ALL` list, and adding a variant becomes a
// one-line change that can't get out of sync.
//
// YOUR TASK
//   1. Make `impl_signal_value!` generate, for every `type => Variant` pair,
//      `impl SignalValue for type` and `impl From<type> for DataValue`.
//   2. Make `define_data_types!` generate `enum DataType` (derive Debug,
//      Clone, Copy, PartialEq, Eq), `DataType::ALL`, `DataType::name()`,
//      `impl Display` and `impl FromStr` (error: `UnknownType(String)`).
//   Do not edit the invocations, `DataValue`, `SignalValue` or the tests.

// I AM NOT DONE

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
enum DataValue {
    Bool(bool),
    Int32(i32),
    Int64(i64),
    Uint32(u32),
    Float(f32),
    Double(f64),
    String(String),
}

trait SignalValue: Sized {
    /// The Rust type's name, e.g. "i32".
    const TYPE_NAME: &'static str;
    fn into_data_value(self) -> DataValue;
    fn from_data_value(value: &DataValue) -> Option<Self>;
}

macro_rules! impl_signal_value {
    ($($t:ty => $variant:ident),* $(,)?) => {
        // TODO: for each pair, generate `impl SignalValue for $t` and
        // `impl From<$t> for DataValue`.
    };
}

#[derive(Debug, PartialEq)]
struct UnknownType(String);

macro_rules! define_data_types {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        // TODO: generate `enum DataType`, `DataType::ALL`, `DataType::name()`,
        // `impl fmt::Display` and `impl FromStr` (Err = UnknownType).
    };
}

impl_signal_value! {
    bool => Bool,
    i32 => Int32,
    i64 => Int64,
    u32 => Uint32,
    f32 => Float,
    f64 => Double,
    String => String,
}

define_data_types! {
    Bool => "boolean",
    Int32 => "int32",
    Int64 => "int64",
    Uint32 => "uint32",
    Float => "float",
    Double => "double",
    String => "string",
    StringArray => "string[]",
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A macro that generates one `#[test]` function per case. Test suites
    /// in real crates (including tokio and serde) use this trick a lot.
    macro_rules! roundtrip_tests {
        ($($test_name:ident: $value:expr => $ty:ty),* $(,)?) => {
            $(
                #[test]
                fn $test_name() {
                    let original: $ty = $value;
                    let dv: DataValue = original.clone().into();
                    assert_eq!(<$ty as SignalValue>::from_data_value(&dv), Some(original));
                }
            )*
        };
    }

    roundtrip_tests! {
        roundtrip_bool: true => bool,
        roundtrip_i32: -40 => i32,
        roundtrip_i64: i64::MAX => i64,
        roundtrip_u32: 55555 => u32,
        roundtrip_f32: 21.5 => f32,
        roundtrip_f64: 1e-9 => f64,
        roundtrip_string: "Vehicle.Speed".to_string() => String,
    }

    #[test]
    fn type_names_come_from_stringify() {
        assert_eq!(<u32 as SignalValue>::TYPE_NAME, "u32");
        assert_eq!(<String as SignalValue>::TYPE_NAME, "String");
    }

    #[test]
    fn wrong_variant_is_none() {
        assert_eq!(i32::from_data_value(&DataValue::Bool(true)), None);
    }

    #[test]
    fn data_type_table() {
        assert_eq!(DataType::ALL.len(), 8);
        assert_eq!(DataType::ALL[0], DataType::Bool);
        assert_eq!(DataType::StringArray.to_string(), "string[]");
        assert_eq!("int64".parse::<DataType>(), Ok(DataType::Int64));
        assert_eq!(
            "int128".parse::<DataType>(),
            Err(UnknownType("int128".to_string()))
        );
    }

    #[test]
    fn display_and_from_str_round_trip_for_every_type() {
        for &ty in DataType::ALL {
            assert_eq!(ty.to_string().parse::<DataType>(), Ok(ty));
        }
    }
}
