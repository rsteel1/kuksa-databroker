// c02_e04_mut_borrow.rs
//
// CONCEPT: mutable borrows `&mut T`, dereferencing with `*`, and the
//          "one writer XOR many readers" rule.
// GUIDE:   chapters/02-ownership/guide.md  (section "Mutable borrows and the one-writer rule")
// DATABROKER: databroker/src/broker.rs — `Entry::apply(&mut self, ...)`
//             changes an entry in place; `DatabaseWriteAccess` holds a
//             `&'a mut Database`, so nobody else can read while it writes.
// EXPECT:  compile errors E0596 (cannot borrow as mutable) and E0502
//          (cannot borrow as mutable because it is also borrowed as immutable).
//
// `&mut T` is an *exclusive* loan: while it exists, nobody else may read or
// write the value. That single rule is what makes data races and "iterator
// invalidation" bugs impossible in safe Rust.
//
// YOUR TASK
//   1. Fix `record` (two separate problems; read both errors).
//   2. Implement `clamp_all`, which changes every reading in place so that it
//      lies within `min..=max`. You'll need `iter_mut()` and `*`.
//   Do not edit the tests.

// I AM NOT DONE

fn push_reading(buffer: &mut Vec<i32>, value: i32) {
    buffer.push(value);
}

/// Records two readings; returns (first reading, total count).
fn record(a: i32, b: i32) -> (i32, usize) {
    let readings = Vec::new();
    push_reading(&mut readings, a);
    let first = &readings[0];
    push_reading(&mut readings, b);
    (*first, readings.len())
}

fn clamp_all(readings: &mut [i32], min: i32, max: i32) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording() {
        assert_eq!(record(5, 9), (5, 2));
    }

    #[test]
    fn clamping() {
        let mut readings = vec![-40, 0, 25, 300];
        clamp_all(&mut readings, -20, 250);
        assert_eq!(readings, vec![-20, 0, 25, 250]);
    }
}
