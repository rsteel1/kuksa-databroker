// c15_e01_size_and_layout.rs
//
// CONCEPT: size, alignment and padding; why field order matters for `#[repr(C)]`
// GUIDE:   chapters/15-advanced-types-and-layout/guide.md  (section "Size, alignment and padding")
// DATABROKER: databroker/src/broker.rs — `Datapoint` / `Metadata` use the default
//             `repr(Rust)`, so the compiler is free to reorder their fields.
// EXPECT:  tests fail: `size_of::<WireHeader>()` is 24, the test wants 16
//
// Every type has a size and an alignment. A `u64` must start at an address
// divisible by 8, so the compiler inserts padding bytes between fields.
// For normal structs (`repr(Rust)`) the compiler reorders fields to keep
// padding small. `#[repr(C)]` promises "fields in declaration order, C rules",
// which you need when the layout is shared with C code or a wire format,
// but then *you* are responsible for a good order.
//
// Assumes a 64-bit target (usize = 8 bytes), like every machine the
// databroker ships on.
//
// YOUR TASK
//   Reorder the fields of `WireHeader` (keep `#[repr(C)]`, keep every field
//   and its type) so the struct shrinks from 24 to 16 bytes.
//   Then fill in `padding_bytes`.
//   Do not edit the tests.

use std::mem::{align_of, size_of};

/// A header written in front of each signal update on a (pretend) wire.
/// `#[repr(C)]`: fields are laid out exactly in the order written here.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct WireHeader {
    // WHY: largest alignment first, then descending: no gaps are needed.
    timestamp_ns: u64,
    signal_id: u32,
    version: u8,
    flags: u8,
}

/// The same fields with the default representation. The compiler may reorder.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RustHeader {
    version: u8,
    timestamp_ns: u64,
    flags: u8,
    signal_id: u32,
}

/// How many bytes of `T` are padding, given the sum of its field sizes.
fn padding_bytes<T>(sum_of_field_sizes: usize) -> usize {
    // WHY: whatever the struct occupies beyond its fields is padding.
    size_of::<T>() - sum_of_field_sizes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    const FIELDS: usize = 8 + 4 + 1 + 1; // u64 + u32 + u8 + u8

    #[test]
    fn repr_c_header_is_compact() {
        assert_eq!(size_of::<WireHeader>(), 16, "reorder the fields");
        assert_eq!(align_of::<WireHeader>(), 8); // the u64 decides
    }

    #[test]
    fn the_u64_is_aligned() {
        assert_eq!(offset_of!(WireHeader, timestamp_ns) % 8, 0);
        assert_eq!(offset_of!(WireHeader, signal_id) % 4, 0);
    }

    #[test]
    fn default_repr_is_already_compact() {
        // The compiler reordered `RustHeader` for you, whatever order you wrote.
        // (`repr(Rust)` layout is unspecified by the language; every current rustc
        // sorts fields to minimise padding, on x86_64 and aarch64 alike.)
        assert_eq!(size_of::<RustHeader>(), 16);
    }

    #[test]
    fn padding_is_what_is_left_over() {
        assert_eq!(padding_bytes::<WireHeader>(FIELDS), 2);
        assert_eq!(padding_bytes::<(u8, u64)>(9), 7);
        assert_eq!(padding_bytes::<[u16; 3]>(6), 0); // arrays never pad between elements
    }

    #[test]
    fn values_still_work() {
        let h = WireHeader {
            timestamp_ns: 1,
            signal_id: 2,
            version: 3,
            flags: 4,
        };
        assert_eq!(h.signal_id + u32::from(h.version) + u32::from(h.flags), 9);
    }
}
