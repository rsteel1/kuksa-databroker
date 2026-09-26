// c02_e01_move.rs
//
// CONCEPT: ownership and moves; `Copy` types vs owned heap data.
// GUIDE:   chapters/02-ownership/guide.md  (section "What a move really is")
// DATABROKER: databroker/src/broker.rs — `Entry::apply` takes `update:
//             EntryUpdate` by value: the caller hands the update over and
//             cannot use it afterwards.
// EXPECT:  compile error E0382 (borrow of moved value: `speed`).
//
// A `String` owns a buffer on the heap. Passing it to a function *by value*
// moves ownership into that function: the old variable is no longer usable.
// An integer is `Copy`: passing it copies the bits and the original stays valid.
//
// YOUR TASK
//   Make `summary` compile WITHOUT calling `.clone()`. Hint: order matters —
//   take what you need from `speed` before you give it away.
//   Do not edit `append_unit` or the tests.

// I AM NOT DONE

/// Takes ownership of `label`, appends the unit, and gives the String back.
fn append_unit(mut label: String, unit: &str) -> String {
    label.push_str(" [");
    label.push_str(unit);
    label.push(']');
    label
}

/// Takes a `u32` by value. `u32` is `Copy`, so the caller keeps its own copy.
fn double(x: u32) -> u32 {
    x * 2
}

/// Returns (length of the path, the labelled path, the doubled id).
fn summary() -> (usize, String, u32) {
    let speed = String::from("Vehicle.Speed");
    let id = 7;

    let with_unit = append_unit(speed, "km/h");
    let path_len = speed.len();

    let doubled = double(id);
    // `id` is still usable here because `u32` is Copy:
    assert_eq!(id, 7);

    (path_len, with_unit, doubled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_is_correct() {
        let (len, labelled, doubled) = summary();
        assert_eq!(len, "Vehicle.Speed".len());
        assert_eq!(labelled, "Vehicle.Speed [km/h]");
        assert_eq!(doubled, 14);
    }
}
